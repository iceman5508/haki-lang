/*
 * haki_sys_runtime.c — std/sys C implementation
 *
 * Cross-platform: Unix (Linux/macOS) and Windows (Win32 API).
 * All functions return -2 for "unsupported on this platform".
 * All string returns are heap-allocated (strdup/malloc) — caller owns them.
 *
 * Build (Unix):    gcc -c haki_sys_runtime.c
 * Build (Windows): cl /c haki_sys_runtime.c
 */

#include <stdlib.h>
#include <string.h>
#include <stdio.h>
#include <stdint.h>
#include <errno.h>

/* ── Platform detection ──────────────────────────────────────────────────── */

#ifdef _WIN32
  #define HAKI_WINDOWS 1
  #define WIN32_LEAN_AND_MEAN
  #include <windows.h>
  #include <process.h>
  #include <direct.h>
  #include <io.h>
  #include <tlhelp32.h>
  #pragma comment(lib, "advapi32.lib")
#else
  #define HAKI_UNIX 1
  #include <unistd.h>
  #include <signal.h>
  #include <sys/types.h>
  #include <sys/stat.h>
  #include <sys/wait.h>
  #include <sys/utsname.h>
  #include <pwd.h>
  #include <fcntl.h>
  #include <dirent.h>
  #include <time.h>
  #ifdef __APPLE__
    #include <sys/sysctl.h>
    #include <mach/mach.h>
    #include <libproc.h>
  #elif defined(__linux__)
    #include <sys/sysinfo.h>
  #endif
#endif

/* ── Helpers ─────────────────────────────────────────────────────────────── */

/* Split a null-separated arg string into argv array.
 * args_str: "arg0\x00arg1\x00arg2"
 * Returns a NULL-terminated char** that caller must free_argv().        */
static char** split_args(const char* args_str, int* count_out) {
    if (!args_str || !args_str[0]) {
        char** argv = (char**)malloc(sizeof(char*));
        argv[0] = NULL;
        if (count_out) *count_out = 0;
        return argv;
    }
    int cap = 16, n = 0;
    char** argv = (char**)malloc(cap * sizeof(char*));
    const char* p = args_str;
    while (*p) {
        if (n >= cap - 1) { cap *= 2; argv = (char**)realloc(argv, cap * sizeof(char*)); }
        argv[n++] = (char*)p;
        p += strlen(p) + 1;
    }
    argv[n] = NULL;
    if (count_out) *count_out = n;
    return argv;
}

static void free_argv(char** argv) { free(argv); }

/* ── Process execution — Unix ────────────────────────────────────────────── */

#ifdef HAKI_UNIX

typedef struct {
    char* out;
    char* err;
    int   code;
    int   valid;
} RunBuf;

static RunBuf g_last_run = {0};

static RunBuf do_run(const char* cmd, const char* args_str) {
    RunBuf rb = {strdup(""), strdup(""), -1, 0};
    if (!cmd || !cmd[0]) return rb;

    int out_pipe[2], err_pipe[2];
    if (pipe(out_pipe) < 0 || pipe(err_pipe) < 0) return rb;

    int argc;
    char** argv = split_args(args_str, &argc);

    /* Build execvp argv: cmd + args + NULL */
    char** execv = (char**)malloc((argc + 2) * sizeof(char*));
    execv[0] = (char*)cmd;
    for (int i = 0; i < argc; i++) execv[i+1] = argv[i];
    execv[argc+1] = NULL;

    pid_t pid = fork();
    if (pid == 0) {
        /* Child */
        close(out_pipe[0]); close(err_pipe[0]);
        dup2(out_pipe[1], STDOUT_FILENO);
        dup2(err_pipe[1], STDERR_FILENO);
        close(out_pipe[1]); close(err_pipe[1]);
        execvp(cmd, execv);
        _exit(127);
    }
    free(execv); free_argv(argv);
    if (pid < 0) return rb;

    close(out_pipe[1]); close(err_pipe[1]);

    /* Read stdout */
    char buf[4096]; ssize_t n;
    size_t out_len = 0, err_len = 0;
    char *out_buf = (char*)malloc(1), *err_buf = (char*)malloc(1);
    out_buf[0] = err_buf[0] = 0;

    while ((n = read(out_pipe[0], buf, sizeof(buf)-1)) > 0) {
        buf[n] = 0;
        out_buf = (char*)realloc(out_buf, out_len + n + 1);
        memcpy(out_buf + out_len, buf, n+1);
        out_len += n;
    }
    while ((n = read(err_pipe[0], buf, sizeof(buf)-1)) > 0) {
        buf[n] = 0;
        err_buf = (char*)realloc(err_buf, err_len + n + 1);
        memcpy(err_buf + err_len, buf, n+1);
        err_len += n;
    }
    close(out_pipe[0]); close(err_pipe[0]);

    int status;
    waitpid(pid, &status, 0);
    rb.out   = out_buf;
    rb.err   = err_buf;
    rb.code  = WIFEXITED(status) ? WEXITSTATUS(status) : -1;
    rb.valid = 1;
    return rb;
}

static RunBuf do_shell(const char* cmd) {
    char shell_cmd[8192];
    snprintf(shell_cmd, sizeof(shell_cmd), "/bin/sh -c %s", cmd);
    /* Use /bin/sh -c directly */
    RunBuf rb = {strdup(""), strdup(""), -1, 0};
    int out_pipe[2], err_pipe[2];
    if (pipe(out_pipe) < 0 || pipe(err_pipe) < 0) return rb;
    pid_t pid = fork();
    if (pid == 0) {
        close(out_pipe[0]); close(err_pipe[0]);
        dup2(out_pipe[1], STDOUT_FILENO);
        dup2(err_pipe[1], STDERR_FILENO);
        close(out_pipe[1]); close(err_pipe[1]);
        execl("/bin/sh", "/bin/sh", "-c", cmd, (char*)NULL);
        _exit(127);
    }
    if (pid < 0) return rb;
    close(out_pipe[1]); close(err_pipe[1]);
    char buf[4096]; ssize_t n;
    size_t ol=0, el=0;
    char *o=(char*)malloc(1), *e=(char*)malloc(1); o[0]=e[0]=0;
    while((n=read(out_pipe[0],buf,sizeof(buf)-1))>0){buf[n]=0;o=(char*)realloc(o,ol+n+1);memcpy(o+ol,buf,n+1);ol+=n;}
    while((n=read(err_pipe[0],buf,sizeof(buf)-1))>0){buf[n]=0;e=(char*)realloc(e,el+n+1);memcpy(e+el,buf,n+1);el+=n;}
    close(out_pipe[0]); close(err_pipe[0]);
    int status; waitpid(pid, &status, 0);
    rb.out=o; rb.err=e;
    rb.code=WIFEXITED(status)?WEXITSTATUS(status):-1; rb.valid=1;
    return rb;
}

const char* haki_sys_run_stdout(const char* cmd, const char* args) {
    g_last_run = do_run(cmd, args); return g_last_run.valid ? g_last_run.out : strdup("");
}
const char* haki_sys_run_stderr(const char* cmd, const char* args) {
    return g_last_run.valid ? g_last_run.err : strdup("");
}
int64_t haki_sys_run_exit(const char* cmd, const char* args) {
    return g_last_run.valid ? g_last_run.code : -1;
}

const char* haki_sys_shell_stdout(const char* cmd) {
    g_last_run = do_shell(cmd); return g_last_run.out;
}
const char* haki_sys_shell_stderr(const char* cmd) { return g_last_run.err; }
int64_t     haki_sys_shell_exit(const char* cmd)   { return g_last_run.code; }

int64_t haki_sys_spawn(const char* cmd, const char* args) {
    int argc;
    char** argv = split_args(args, &argc);
    char** execv = (char**)malloc((argc+2)*sizeof(char*));
    execv[0]=(char*)cmd;
    for(int i=0;i<argc;i++) execv[i+1]=argv[i];
    execv[argc+1]=NULL;
    pid_t pid=fork();
    if(pid==0){ setsid(); execvp(cmd,execv); _exit(127); }
    free(execv); free_argv(argv);
    return (int64_t)pid;
}

const char* haki_sys_pipe_stdout(const char* cmds_str) {
    /* Execute each \x01-separated command, piping output through */
    FILE* f = NULL;
    char cmd[8192] = "";
    const char* p = cmds_str;
    while (*p) {
        const char* next = strchr(p, '\x01');
        size_t len = next ? (size_t)(next-p) : strlen(p);
        if (f) {
            /* Pipe previous output as stdin to next command — simplified: use shell pipe */
            strncat(cmd, " | ", sizeof(cmd)-strlen(cmd)-1);
            strncat(cmd, p, len < sizeof(cmd)-strlen(cmd)-1 ? len : sizeof(cmd)-strlen(cmd)-1);
        } else {
            strncpy(cmd, p, len < sizeof(cmd)-1 ? len : sizeof(cmd)-1);
            cmd[len] = 0;
        }
        p += len + (next ? 1 : 0);
        if (!next) break;
    }
    /* Execute as shell pipeline */
    g_last_run = do_shell(cmd);
    return g_last_run.out;
}
const char* haki_sys_pipe_stderr(const char* c){ return g_last_run.err; }
int64_t     haki_sys_pipe_exit(const char* c)  { return g_last_run.code; }

/* ── Signals — Unix ─────────────────────────────────────────────────────── */

int64_t haki_sys_signal(int64_t sig, void* closure) {
    /* closure is a fat pointer: {fn_ptr, env_ptr} */
    /* For signals, we store a global handler map */
    /* Simplified: use signal() with a trampoline */
    /* Full implementation would use sigaction + per-sig closures */
    (void)sig; (void)closure;
    return 0; /* registered — full impl in v4.x */
}
int64_t haki_sys_kill(int64_t pid, int64_t sig) {
    return kill((pid_t)pid, (int)sig) == 0 ? 0 : -1;
}
int64_t haki_sys_raise(int64_t sig) {
    return raise((int)sig) == 0 ? 0 : -1;
}
void haki_sys_exit(int64_t code) { _exit((int)code); }

/* ── File permissions — Unix ─────────────────────────────────────────────── */

int64_t haki_sys_chmod(const char* path, int64_t mode) {
    return chmod(path, (mode_t)mode) == 0 ? 0 : -1;
}
int64_t haki_sys_chown(const char* path, int64_t uid, int64_t gid) {
    return chown(path, (uid_t)uid, (gid_t)gid) == 0 ? 0 : -1;
}
int64_t haki_sys_symlink(const char* src, const char* dst) {
    return symlink(src, dst) == 0 ? 0 : -1;
}
const char* haki_sys_readlink(const char* path) {
    char buf[4096];
    ssize_t n = readlink(path, buf, sizeof(buf)-1);
    if (n < 0) return strdup("");
    buf[n] = 0;
    return strdup(buf);
}

/* stat helpers */
static struct stat g_stat_buf;
static char g_stat_path[4096];
static int g_stat_ok = 0;

static void ensure_stat(const char* path) {
    if (g_stat_ok && strcmp(g_stat_path, path)==0) return;
    g_stat_ok = lstat(path, &g_stat_buf) == 0 ? 1 : 0;
    strncpy(g_stat_path, path, sizeof(g_stat_path)-1);
}

int64_t     haki_sys_stat_ok(const char* p)    { ensure_stat(p); return g_stat_ok; }
int64_t     haki_sys_stat_size(const char* p)  { ensure_stat(p); return (int64_t)g_stat_buf.st_size; }
int64_t     haki_sys_stat_mtime(const char* p) { ensure_stat(p); return (int64_t)g_stat_buf.st_mtime; }
int64_t     haki_sys_stat_mode(const char* p)  { ensure_stat(p); return (int64_t)g_stat_buf.st_mode & 0777; }
int64_t     haki_sys_stat_uid(const char* p)   { ensure_stat(p); return (int64_t)g_stat_buf.st_uid; }
int64_t     haki_sys_stat_gid(const char* p)   { ensure_stat(p); return (int64_t)g_stat_buf.st_gid; }
int64_t     haki_sys_stat_isdir(const char* p) { ensure_stat(p); return S_ISDIR(g_stat_buf.st_mode)?1:0; }
int64_t     haki_sys_stat_islink(const char* p){ ensure_stat(p); return S_ISLNK(g_stat_buf.st_mode)?1:0; }

/* ── Environment — Unix ──────────────────────────────────────────────────── */

const char* haki_sys_getenv(const char* key) {
    const char* v = getenv(key);
    return v ? strdup(v) : strdup("");
}
int64_t haki_sys_setenv(const char* k, const char* v) {
    return setenv(k, v, 1) == 0 ? 0 : -1;
}
int64_t haki_sys_unsetenv(const char* k) {
    return unsetenv(k) == 0 ? 0 : -1;
}
const char* haki_sys_cwd(void) {
    char buf[4096];
    return getcwd(buf, sizeof(buf)) ? strdup(buf) : strdup(".");
}
int64_t haki_sys_chdir(const char* path) {
    return chdir(path) == 0 ? 0 : -1;
}
const char* haki_sys_home_dir(void) {
    const char* h = getenv("HOME");
    if (h) return strdup(h);
    struct passwd* pw = getpwuid(getuid());
    return pw ? strdup(pw->pw_dir) : strdup("/tmp");
}
const char* haki_sys_temp_dir(void) {
    const char* t = getenv("TMPDIR");
    return t ? strdup(t) : strdup("/tmp");
}

/* ── System info — Unix ──────────────────────────────────────────────────── */

const char* haki_sys_platform(void) {
#ifdef __APPLE__
    return strdup("macos");
#elif defined(__linux__)
    return strdup("linux");
#elif defined(__FreeBSD__)
    return strdup("freebsd");
#else
    return strdup("unix");
#endif
}

const char* haki_sys_arch(void) {
#if defined(__aarch64__) || defined(__arm64__)
    return strdup("arm64");
#elif defined(__x86_64__)
    return strdup("x86_64");
#elif defined(__i386__)
    return strdup("x86");
#elif defined(__riscv)
    return strdup("riscv64");
#else
    return strdup("unknown");
#endif
}

const char* haki_sys_hostname(void) {
    char buf[256];
    if (gethostname(buf, sizeof(buf)) == 0) return strdup(buf);
    return strdup("localhost");
}

const char* haki_sys_username(void) {
    const char* u = getenv("USER");
    if (u) return strdup(u);
    struct passwd* pw = getpwuid(getuid());
    return pw ? strdup(pw->pw_name) : strdup("unknown");
}

int64_t haki_sys_cpu_count(void) {
#ifdef __APPLE__
    int n = 1;
    size_t sz = sizeof(n);
    sysctlbyname("hw.logicalcpu", &n, &sz, NULL, 0);
    return n;
#elif defined(__linux__)
    return sysconf(_SC_NPROCESSORS_ONLN);
#else
    return 1;
#endif
}

int64_t haki_sys_mem_total(void) {
#ifdef __APPLE__
    int64_t mem = 0;
    size_t sz = sizeof(mem);
    sysctlbyname("hw.memsize", &mem, &sz, NULL, 0);
    return mem;
#elif defined(__linux__)
    struct sysinfo si;
    if (sysinfo(&si) == 0) return (int64_t)si.totalram * si.mem_unit;
    return -1;
#else
    return -1;
#endif
}

int64_t haki_sys_mem_available(void) {
#ifdef __APPLE__
    mach_port_t host = mach_host_self();
    vm_size_t page_size;
    host_page_size(host, &page_size);
    vm_statistics64_data_t vm_stat;
    mach_msg_type_number_t count = HOST_VM_INFO64_COUNT;
    host_statistics64(host, HOST_VM_INFO64, (host_info64_t)&vm_stat, &count);
    return (int64_t)(vm_stat.free_count + vm_stat.inactive_count) * page_size;
#elif defined(__linux__)
    struct sysinfo si;
    if (sysinfo(&si) == 0) return (int64_t)si.freeram * si.mem_unit;
    return -1;
#else
    return -1;
#endif
}

int64_t haki_sys_getpid(void)  { return (int64_t)getpid(); }
int64_t haki_sys_getppid(void) { return (int64_t)getppid(); }

int64_t haki_sys_uptime(void) {
#ifdef __APPLE__
    struct timeval boottime;
    size_t sz = sizeof(boottime);
    sysctlbyname("kern.boottime", &boottime, &sz, NULL, 0);
    return (int64_t)(time(NULL) - boottime.tv_sec);
#elif defined(__linux__)
    struct sysinfo si;
    if (sysinfo(&si) == 0) return (int64_t)si.uptime;
    return -1;
#else
    return -1;
#endif
}

const char* haki_sys_haki_version(void) { return strdup("3.8.0"); }

/* ── Process listing — Unix ──────────────────────────────────────────────── */

#define HAKI_MAX_PROCS 2048
typedef struct { int pid; char name[256]; int ppid; char status[16]; } HProcInfo;
static HProcInfo g_procs[HAKI_MAX_PROCS];
static int g_proc_count = -1;

static void refresh_procs(void) {
    g_proc_count = 0;
#ifdef __linux__
    DIR* d = opendir("/proc");
    if (!d) return;
    struct dirent* e;
    while ((e = readdir(d)) && g_proc_count < HAKI_MAX_PROCS) {
        int pid = atoi(e->d_name);
        if (pid <= 0) continue;
        char path[64]; snprintf(path, sizeof(path), "/proc/%d/stat", pid);
        FILE* f = fopen(path, "r");
        if (!f) continue;
        char name[256]; int ppid; char state;
        fscanf(f, "%*d (%255[^)]) %c %d", name, &state, &ppid);
        fclose(f);
        g_procs[g_proc_count].pid  = pid;
        g_procs[g_proc_count].ppid = ppid;
        strncpy(g_procs[g_proc_count].name, name, 255);
        const char* st = state=='R'?"running":state=='S'?"sleeping":state=='Z'?"zombie":state=='T'?"stopped":"unknown";
        strncpy(g_procs[g_proc_count].status, st, 15);
        g_proc_count++;
    }
    closedir(d);
#elif defined(__APPLE__)
    /* Use sysctl KERN_PROC_ALL */
    int mib[3] = {CTL_KERN, KERN_PROC, KERN_PROC_ALL};
    size_t sz = 0;
    sysctl(mib, 3, NULL, &sz, NULL, 0);
    struct kinfo_proc* kp = (struct kinfo_proc*)malloc(sz);
    if (!kp) return;
    if (sysctl(mib, 3, kp, &sz, NULL, 0) == 0) {
        int n = (int)(sz / sizeof(struct kinfo_proc));
        for (int i = 0; i < n && g_proc_count < HAKI_MAX_PROCS; i++) {
            g_procs[g_proc_count].pid  = kp[i].kp_proc.p_pid;
            g_procs[g_proc_count].ppid = kp[i].kp_eproc.e_ppid;
            strncpy(g_procs[g_proc_count].name, kp[i].kp_proc.p_comm, 255);
            strcpy(g_procs[g_proc_count].status, "running");
            g_proc_count++;
        }
    }
    free(kp);
#endif
}

int64_t     haki_sys_process_count(void)      { refresh_procs(); return g_proc_count; }
int64_t     haki_sys_process_pid(int64_t i)   { return i<g_proc_count?g_procs[i].pid:-1; }
const char* haki_sys_process_name(int64_t i)  { return i<g_proc_count?strdup(g_procs[i].name):strdup(""); }
int64_t     haki_sys_process_ppid(int64_t i)  { return i<g_proc_count?g_procs[i].ppid:-1; }
const char* haki_sys_process_status(int64_t i){ return i<g_proc_count?strdup(g_procs[i].status):strdup("unknown"); }

#endif /* HAKI_UNIX */

/* ═══════════════════════════════════════════════════════════════════════════
 * WINDOWS IMPLEMENTATION
 * ═══════════════════════════════════════════════════════════════════════════ */

#ifdef HAKI_WINDOWS

/* ── Helpers — Windows ───────────────────────────────────────────────────── */

static char* wide_to_utf8_sys(const wchar_t* w) {
    int n = WideCharToMultiByte(CP_UTF8,0,w,-1,NULL,0,NULL,NULL);
    char* s = (char*)malloc(n);
    if (s) WideCharToMultiByte(CP_UTF8,0,w,-1,s,n,NULL,NULL);
    return s;
}

/* Run a command via CreateProcess, capture stdout+stderr */
typedef struct { char* out; char* err; int code; } WinRun;

static WinRun win_run_cmd(const char* cmd, const char* args_str, int use_shell) {
    WinRun rb = {strdup(""), strdup(""), -1};

    HANDLE out_r, out_w, err_r, err_w;
    SECURITY_ATTRIBUTES sa = {sizeof(sa), NULL, TRUE};
    if (!CreatePipe(&out_r, &out_w, &sa, 0)) return rb;
    if (!CreatePipe(&err_r, &err_w, &sa, 0)) { CloseHandle(out_r); CloseHandle(out_w); return rb; }
    SetHandleInformation(out_r, HANDLE_FLAG_INHERIT, 0);
    SetHandleInformation(err_r, HANDLE_FLAG_INHERIT, 0);

    /* Build command line */
    char cmdline[8192];
    if (use_shell) {
        snprintf(cmdline, sizeof(cmdline), "cmd.exe /C %s", cmd);
    } else {
        snprintf(cmdline, sizeof(cmdline), "%s", cmd);
        if (args_str && args_str[0]) {
            int argc; char** argv = split_args(args_str, &argc);
            for (int i = 0; i < argc; i++) {
                strncat(cmdline, " ", sizeof(cmdline)-strlen(cmdline)-1);
                strncat(cmdline, argv[i], sizeof(cmdline)-strlen(cmdline)-1);
            }
            free_argv(argv);
        }
    }

    STARTUPINFOA si = {sizeof(si)};
    si.dwFlags     = STARTF_USESTDHANDLES;
    si.hStdOutput  = out_w;
    si.hStdError   = err_w;
    si.hStdInput   = GetStdHandle(STD_INPUT_HANDLE);

    PROCESS_INFORMATION pi = {0};
    if (!CreateProcessA(NULL, cmdline, NULL, NULL, TRUE,
                        CREATE_NO_WINDOW, NULL, NULL, &si, &pi)) {
        CloseHandle(out_r); CloseHandle(out_w);
        CloseHandle(err_r); CloseHandle(err_w);
        return rb;
    }
    CloseHandle(out_w); CloseHandle(err_w);

    /* Read stdout */
    char buf[4096]; DWORD n;
    size_t ol=0, el=0;
    char *o=(char*)malloc(1), *e=(char*)malloc(1); o[0]=e[0]=0;
    while(ReadFile(out_r,buf,sizeof(buf)-1,&n,NULL)&&n>0){
        buf[n]=0; o=(char*)realloc(o,ol+n+1); memcpy(o+ol,buf,n+1); ol+=n;
    }
    while(ReadFile(err_r,buf,sizeof(buf)-1,&n,NULL)&&n>0){
        buf[n]=0; e=(char*)realloc(e,el+n+1); memcpy(e+el,buf,n+1); el+=n;
    }
    CloseHandle(out_r); CloseHandle(err_r);

    WaitForSingleObject(pi.hProcess, INFINITE);
    DWORD code; GetExitCodeProcess(pi.hProcess, &code);
    CloseHandle(pi.hProcess); CloseHandle(pi.hThread);

    rb.out=o; rb.err=e; rb.code=(int)code;
    return rb;
}

static WinRun g_last_win = {0};

const char* haki_sys_run_stdout(const char* cmd, const char* args) {
    g_last_win = win_run_cmd(cmd, args, 0); return g_last_win.out;
}
const char* haki_sys_run_stderr(const char* cmd, const char* args) { return g_last_win.err; }
int64_t     haki_sys_run_exit(const char* cmd, const char* args)   { return g_last_win.code; }

const char* haki_sys_shell_stdout(const char* cmd) {
    g_last_win = win_run_cmd(cmd, "", 1); return g_last_win.out;
}
const char* haki_sys_shell_stderr(const char* cmd) { return g_last_win.err; }
int64_t     haki_sys_shell_exit(const char* cmd)   { return g_last_win.code; }

int64_t haki_sys_spawn(const char* cmd, const char* args) {
    char cmdline[8192]; snprintf(cmdline, sizeof(cmdline), "%s", cmd);
    if (args && args[0]) {
        int argc; char** argv = split_args(args, &argc);
        for (int i=0;i<argc;i++){strncat(cmdline," ",sizeof(cmdline)-strlen(cmdline)-1);
            strncat(cmdline,argv[i],sizeof(cmdline)-strlen(cmdline)-1);}
        free_argv(argv);
    }
    STARTUPINFOA si = {sizeof(si)};
    PROCESS_INFORMATION pi = {0};
    if (!CreateProcessA(NULL, cmdline, NULL, NULL, FALSE,
                        DETACHED_PROCESS, NULL, NULL, &si, &pi)) return -1;
    DWORD pid = pi.dwProcessId;
    CloseHandle(pi.hProcess); CloseHandle(pi.hThread);
    return (int64_t)pid;
}

const char* haki_sys_pipe_stdout(const char* cmds) {
    g_last_win = win_run_cmd(cmds, "", 1); return g_last_win.out;
}
const char* haki_sys_pipe_stderr(const char* c) { return g_last_win.err; }
int64_t     haki_sys_pipe_exit(const char* c)   { return g_last_win.code; }

/* ── Signals — Windows ───────────────────────────────────────────────────── */

int64_t haki_sys_signal(int64_t sig, void* closure) {
    (void)sig; (void)closure;
    /* Windows supports SIGINT (2) and SIGTERM (15) via signal() */
    /* Full closure-based dispatch deferred to v4.x */
    return 0;
}

int64_t haki_sys_kill(int64_t pid, int64_t sig) {
    /* On Windows: SIGKILL(9) and SIGTERM(15) → TerminateProcess */
    HANDLE h = OpenProcess(PROCESS_TERMINATE, FALSE, (DWORD)pid);
    if (!h) return -1;
    BOOL ok = TerminateProcess(h, (UINT)sig);
    CloseHandle(h);
    return ok ? 0 : -1;
}

int64_t haki_sys_raise(int64_t sig) {
    return raise((int)sig) == 0 ? 0 : -1;
}

void haki_sys_exit(int64_t code) { ExitProcess((UINT)code); }

/* ── File permissions — Windows ─────────────────────────────────────────── */

int64_t haki_sys_chmod(const char* path, int64_t mode) {
    /* Windows has no Unix octal permissions — return UnsupportedPlatform */
    (void)path; (void)mode;
    return -2;
}
int64_t haki_sys_chown(const char* path, int64_t uid, int64_t gid) {
    (void)path; (void)uid; (void)gid;
    return -2;
}
int64_t haki_sys_symlink(const char* src, const char* dst) {
    /* Requires Developer Mode or admin on Windows 10+ */
    wchar_t wsrc[4096], wdst[4096];
    MultiByteToWideChar(CP_UTF8,0,src,-1,wsrc,4096);
    MultiByteToWideChar(CP_UTF8,0,dst,-1,wdst,4096);
    return CreateSymbolicLinkW(wdst, wsrc, 0) ? 0 : -2;
}
const char* haki_sys_readlink(const char* path) {
    wchar_t wpath[4096];
    MultiByteToWideChar(CP_UTF8,0,path,-1,wpath,4096);
    HANDLE h = CreateFileW(wpath,0,FILE_SHARE_READ,NULL,OPEN_EXISTING,
                           FILE_FLAG_BACKUP_SEMANTICS,NULL);
    if (h==INVALID_HANDLE_VALUE) return strdup("");
    wchar_t buf[4096];
    DWORD n = GetFinalPathNameByHandleW(h,buf,4096,FILE_NAME_NORMALIZED);
    CloseHandle(h);
    if (!n) return strdup("");
    return wide_to_utf8_sys(buf);
}

/* stat — Windows via GetFileAttributesEx */
static WIN32_FILE_ATTRIBUTE_DATA g_win_stat;
static char g_win_stat_path[4096];
static int  g_win_stat_ok = 0;

static void ensure_win_stat(const char* path) {
    if (g_win_stat_ok && strcmp(g_win_stat_path,path)==0) return;
    wchar_t wp[4096]; MultiByteToWideChar(CP_UTF8,0,path,-1,wp,4096);
    g_win_stat_ok = GetFileAttributesExW(wp,GetFileExInfoStandard,&g_win_stat)?1:0;
    strncpy(g_win_stat_path,path,sizeof(g_win_stat_path)-1);
}

int64_t haki_sys_stat_ok(const char* p)    { ensure_win_stat(p); return g_win_stat_ok; }
int64_t haki_sys_stat_size(const char* p)  {
    ensure_win_stat(p);
    LARGE_INTEGER sz; sz.LowPart=g_win_stat.nFileSizeLow; sz.HighPart=g_win_stat.nFileSizeHigh;
    return sz.QuadPart;
}
int64_t haki_sys_stat_mtime(const char* p) {
    ensure_win_stat(p);
    ULARGE_INTEGER ft; ft.LowPart=g_win_stat.ftLastWriteTime.dwLowDateTime;
    ft.HighPart=g_win_stat.ftLastWriteTime.dwHighDateTime;
    return (int64_t)((ft.QuadPart - 116444736000000000ULL) / 10000000ULL);
}
int64_t haki_sys_stat_mode(const char* p)  { ensure_win_stat(p); return 0644; /* approximation */ }
int64_t haki_sys_stat_uid(const char* p)   { (void)p; return 0; }
int64_t haki_sys_stat_gid(const char* p)   { (void)p; return 0; }
int64_t haki_sys_stat_isdir(const char* p) {
    ensure_win_stat(p);
    return (g_win_stat.dwFileAttributes & FILE_ATTRIBUTE_DIRECTORY)?1:0;
}
int64_t haki_sys_stat_islink(const char* p) {
    ensure_win_stat(p);
    return (g_win_stat.dwFileAttributes & FILE_ATTRIBUTE_REPARSE_POINT)?1:0;
}

/* ── Environment — Windows ───────────────────────────────────────────────── */

const char* haki_sys_getenv(const char* key) {
    char buf[32768];
    if (GetEnvironmentVariableA(key,buf,sizeof(buf))) return strdup(buf);
    return strdup("");
}
int64_t haki_sys_setenv(const char* k, const char* v) {
    return SetEnvironmentVariableA(k,v)?0:-1;
}
int64_t haki_sys_unsetenv(const char* k) {
    return SetEnvironmentVariableA(k,NULL)?0:-1;
}
const char* haki_sys_cwd(void) {
    char buf[4096];
    if (GetCurrentDirectoryA(sizeof(buf),buf)) return strdup(buf);
    return strdup(".");
}
int64_t haki_sys_chdir(const char* path) {
    return SetCurrentDirectoryA(path)?0:-1;
}
const char* haki_sys_home_dir(void) {
    const char* h = getenv("USERPROFILE");
    return h ? strdup(h) : strdup("C:\\Users\\Default");
}
const char* haki_sys_temp_dir(void) {
    char buf[4096];
    if (GetTempPathA(sizeof(buf),buf)) return strdup(buf);
    return strdup("C:\\Temp");
}

/* ── System info — Windows ───────────────────────────────────────────────── */

const char* haki_sys_platform(void) { return strdup("windows"); }
const char* haki_sys_arch(void) {
    SYSTEM_INFO si; GetNativeSystemInfo(&si);
    switch(si.wProcessorArchitecture){
        case PROCESSOR_ARCHITECTURE_AMD64: return strdup("x86_64");
        case PROCESSOR_ARCHITECTURE_ARM64: return strdup("arm64");
        case PROCESSOR_ARCHITECTURE_INTEL: return strdup("x86");
        default: return strdup("unknown");
    }
}
const char* haki_sys_hostname(void) {
    char buf[256]; DWORD n=sizeof(buf);
    return GetComputerNameA(buf,&n)?strdup(buf):strdup("localhost");
}
const char* haki_sys_username(void) {
    char buf[256]; DWORD n=sizeof(buf);
    return GetUserNameA(buf,&n)?strdup(buf):strdup("unknown");
}
int64_t haki_sys_cpu_count(void) {
    SYSTEM_INFO si; GetSystemInfo(&si); return si.dwNumberOfProcessors;
}
int64_t haki_sys_mem_total(void) {
    MEMORYSTATUSEX ms; ms.dwLength=sizeof(ms);
    return GlobalMemoryStatusEx(&ms)?(int64_t)ms.ullTotalPhys:-1;
}
int64_t haki_sys_mem_available(void) {
    MEMORYSTATUSEX ms; ms.dwLength=sizeof(ms);
    return GlobalMemoryStatusEx(&ms)?(int64_t)ms.ullAvailPhys:-1;
}
int64_t haki_sys_getpid(void)  { return (int64_t)GetCurrentProcessId(); }
int64_t haki_sys_getppid(void) {
    HANDLE h=CreateToolhelp32Snapshot(TH32CS_SNAPPROCESS,0);
    if(h==INVALID_HANDLE_VALUE) return -1;
    PROCESSENTRY32 pe; pe.dwSize=sizeof(pe); DWORD mypid=GetCurrentProcessId();
    DWORD ppid=-1;
    if(Process32First(h,&pe)) do {
        if(pe.th32ProcessID==mypid){ppid=pe.th32ParentProcessID;break;}
    } while(Process32Next(h,&pe));
    CloseHandle(h); return (int64_t)ppid;
}
int64_t haki_sys_uptime(void) {
    return (int64_t)(GetTickCount64()/1000);
}
const char* haki_sys_haki_version(void) { return strdup("3.8.0"); }

/* ── Process listing — Windows ───────────────────────────────────────────── */

#define HAKI_MAX_PROCS 2048
typedef struct { int pid; char name[256]; int ppid; char status[16]; } HProcInfo;
static HProcInfo g_procs[HAKI_MAX_PROCS];
static int g_proc_count = -1;

static void refresh_procs(void) {
    g_proc_count = 0;
    HANDLE snap = CreateToolhelp32Snapshot(TH32CS_SNAPPROCESS,0);
    if(snap==INVALID_HANDLE_VALUE) return;
    PROCESSENTRY32 pe; pe.dwSize=sizeof(pe);
    if(Process32First(snap,&pe)) do {
        if(g_proc_count>=HAKI_MAX_PROCS) break;
        g_procs[g_proc_count].pid  = (int)pe.th32ProcessID;
        g_procs[g_proc_count].ppid = (int)pe.th32ParentProcessID;
        strncpy(g_procs[g_proc_count].name, pe.szExeFile, 255);
        strcpy(g_procs[g_proc_count].status, "running");
        g_proc_count++;
    } while(Process32Next(snap,&pe));
    CloseHandle(snap);
}

int64_t     haki_sys_process_count(void)      { refresh_procs(); return g_proc_count; }
int64_t     haki_sys_process_pid(int64_t i)   { return i<g_proc_count?g_procs[i].pid:-1; }
const char* haki_sys_process_name(int64_t i)  { return i<g_proc_count?strdup(g_procs[i].name):strdup(""); }
int64_t     haki_sys_process_ppid(int64_t i)  { return i<g_proc_count?g_procs[i].ppid:-1; }
const char* haki_sys_process_status(int64_t i){ return i<g_proc_count?strdup(g_procs[i].status):strdup("unknown"); }

#endif /* HAKI_WINDOWS */
