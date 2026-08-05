/*
 * haki_ui_win32.c — Haki UI platform layer for Windows (Win32 API)
 *
 * Implements the exact same integer node_id FFI boundary as haki_ui_gtk.c.
 * The Haki VNode diff engine calls these functions with integer IDs only —
 * no Haki memory ever crosses this boundary.
 *
 * Build: cl /c haki_ui_win32.c /link user32.lib comctl32.lib
 *        or: gcc -c haki_ui_win32.c -luser32 -lcomctl32
 *
 * Requires: Windows XP SP3 or later (tested on Windows 10/11)
 * Dependencies: NONE (pure Win32 API, ships with every Windows installation)
 */

#ifndef _WIN32
#error "haki_ui_win32.c is Windows-only. Use haki_ui_gtk.c on Linux/macOS."
#endif

#define WIN32_LEAN_AND_MEAN
#define UNICODE
#define _UNICODE
#include <windows.h>
#include <commctrl.h>
#include <stdint.h>
#include <stdlib.h>
#include <string.h>
#include <stdio.h>
#include <wchar.h>

/* Enable visual styles (themed controls) via manifest */
#pragma comment(linker,"/manifestdependency:\"type='win32' name='Microsoft.Windows.Common-Controls' version='6.0.0.0' processorArchitecture='*' publicKeyToken='6595b64144ccf1df' language='*'\"")
#pragma comment(lib, "user32.lib")
#pragma comment(lib, "comctl32.lib")

/* ── Node ID registry ────────────────────────────────────────────────────── */

#define HAKI_MAX_NODES 8192

static HWND  g_node_id_map[HAKI_MAX_NODES];  /* VNode id → HWND           */
static int64_t g_next_id = 1;
static HWND  g_window = NULL;                 /* top-level window          */

/* Layout tracking for Column/Row containers */
typedef struct {
    int is_row;          /* 1=horizontal, 0=vertical */
    HWND children[256];
    int  child_count;
    int  padding;
} HakiBox;
static HakiBox g_boxes[HAKI_MAX_NODES];

/* ── UTF-8 ↔ UTF-16 helpers ─────────────────────────────────────────────── */

static wchar_t* utf8_to_wide(const char* s) {
    if (!s) return _wcsdup(L"");
    int len = MultiByteToWideChar(CP_UTF8, 0, s, -1, NULL, 0);
    wchar_t* w = (wchar_t*)malloc(len * sizeof(wchar_t));
    if (w) MultiByteToWideChar(CP_UTF8, 0, s, -1, w, len);
    return w;
}

static char* wide_to_utf8(const wchar_t* w) {
    if (!w) return _strdup("");
    int len = WideCharToMultiByte(CP_UTF8, 0, w, -1, NULL, 0, NULL, NULL);
    char* s = (char*)malloc(len);
    if (s) WideCharToMultiByte(CP_UTF8, 0, w, -1, s, len, NULL, NULL);
    return s;
}

/* ── Callback registry ──────────────────────────────────────────────────── */

#define HAKI_MAX_CALLBACKS 4096
typedef void (*HakiVoidFn)(void*);
static HakiVoidFn g_callbacks[HAKI_MAX_CALLBACKS];
static void*      g_callback_envs[HAKI_MAX_CALLBACKS];

void haki_register_callback(int64_t node_id, void* closure) {
    if (node_id <= 0 || node_id >= HAKI_MAX_CALLBACKS || !closure) return;
    void** fat = (void**)closure;
    g_callbacks[node_id]     = (HakiVoidFn)fat[0];
    g_callback_envs[node_id] = fat[1];
}

static void fire_callback(int64_t node_id) {
    if (node_id <= 0 || node_id >= HAKI_MAX_CALLBACKS) return;
    if (g_callbacks[node_id]) {
        g_callbacks[node_id](g_callback_envs[node_id]);
    }
}

/* ── Rerender fn (diff engine) ──────────────────────────────────────────── */

static HakiVoidFn g_rerender_fn  = NULL;
static void*      g_rerender_env = NULL;

void haki_set_rerender_fn(void* closure) {
    if (!closure) return;
    void** fat = (void**)closure;
    g_rerender_fn  = (HakiVoidFn)fat[0];
    g_rerender_env = fat[1];
}

void haki_trigger_rerender(void) {
    if (g_rerender_fn) {
        g_rerender_fn(g_rerender_env);
        /* Force repaint of all windows */
        if (g_window) InvalidateRect(g_window, NULL, TRUE);
    }
}

/* Legacy single-label rerender (v3.1 compat) */
void haki_set_rerender_callback(int64_t label_id, void* closure) {
    (void)label_id; (void)closure; /* superseded by diff engine */
}

/* ── Layout engine ─────────────────────────────────────────────────────── */

static void haki_reflow_box(int64_t box_id) {
    if (box_id <= 0 || box_id >= HAKI_MAX_NODES) return;
    HWND parent = g_node_id_map[box_id];
    if (!parent) return;
    HakiBox* box = &g_boxes[box_id];
    if (box->child_count == 0) return;

    RECT rc;
    GetClientRect(parent, &rc);
    int W = rc.right  - rc.left;
    int H = rc.bottom - rc.top;
    int pad = box->padding;
    int n = box->child_count;

    if (box->is_row) {
        /* Horizontal: divide width equally */
        int slot_w = (W - pad * (n + 1)) / n;
        for (int i = 0; i < n; i++) {
            int x = pad + i * (slot_w + pad);
            SetWindowPos(box->children[i], NULL, x, pad, slot_w, H - 2*pad,
                         SWP_NOZORDER | SWP_NOACTIVATE);
        }
    } else {
        /* Vertical: divide height equally */
        int slot_h = (H - pad * (n + 1)) / n;
        for (int i = 0; i < n; i++) {
            int y = pad + i * (slot_h + pad);
            SetWindowPos(box->children[i], NULL, pad, y, W - 2*pad, slot_h,
                         SWP_NOZORDER | SWP_NOACTIVATE);
        }
    }
}

/* ── WndProc ────────────────────────────────────────────────────────────── */

static LRESULT CALLBACK haki_wndproc(HWND hwnd, UINT msg, WPARAM wp, LPARAM lp) {
    switch (msg) {
    case WM_SIZE:
        /* Reflow all top-level boxes when window is resized */
        for (int64_t i = 1; i < g_next_id; i++) {
            if (g_node_id_map[i] && GetParent(g_node_id_map[i]) == hwnd) {
                /* Check if it's a container */
                if (g_boxes[i].child_count > 0) {
                    haki_reflow_box(i);
                }
            }
        }
        return 0;

    case WM_COMMAND:
        /* Button/checkbox/menu clicks come here */
        if (HIWORD(wp) == BN_CLICKED || HIWORD(wp) == 0) {
            int64_t node_id = (int64_t)GetWindowLongPtrW((HWND)lp, GWLP_ID);
            fire_callback(node_id);
            /* Trigger rerender after any button click */
            if (g_rerender_fn) {
                PostMessage(hwnd, WM_USER + 1, 0, 0);
            }
        }
        return 0;

    case WM_USER + 1:
        /* Deferred rerender — safe to call Haki here */
        haki_trigger_rerender();
        return 0;

    case WM_DESTROY:
        PostQuitMessage(0);
        return 0;

    default:
        return DefWindowProcW(hwnd, msg, wp, lp);
    }
}

/* ── Window creation ────────────────────────────────────────────────────── */

static void init_common_controls(void) {
    INITCOMMONCONTROLSEX icc = { sizeof(icc), ICC_WIN95_CLASSES | ICC_STANDARD_CLASSES };
    InitCommonControlsEx(&icc);
}

static const wchar_t* HAKI_WNDCLASS = L"HakiAppWindow";

static void register_wndclass(void) {
    WNDCLASSEXW wc = {0};
    wc.cbSize        = sizeof(wc);
    wc.style         = CS_HREDRAW | CS_VREDRAW;
    wc.lpfnWndProc   = haki_wndproc;
    wc.hInstance     = GetModuleHandleW(NULL);
    wc.hCursor       = LoadCursorW(NULL, IDC_ARROW);
    wc.hbrBackground = (HBRUSH)(COLOR_WINDOW + 1);
    wc.lpszClassName = HAKI_WNDCLASS;
    wc.hIcon         = LoadIconW(NULL, IDI_APPLICATION);
    RegisterClassExW(&wc);
}

int64_t haki_gtk_create_window(const char* title, int64_t width, int64_t height) {
    init_common_controls();
    register_wndclass();

    wchar_t* wtitle = utf8_to_wide(title ? title : "Haki App");
    g_window = CreateWindowExW(
        0, HAKI_WNDCLASS, wtitle,
        WS_OVERLAPPEDWINDOW,
        CW_USEDEFAULT, CW_USEDEFAULT,
        (int)width > 0 ? (int)width : 800,
        (int)height > 0 ? (int)height : 600,
        NULL, NULL, GetModuleHandleW(NULL), NULL
    );
    free(wtitle);

    int64_t id = g_next_id++;
    g_node_id_map[id] = g_window;
    return id;
}

/* ── Widget creation ────────────────────────────────────────────────────── */

static HWND get_parent_hwnd(int64_t parent_id) {
    if (parent_id > 0 && parent_id < HAKI_MAX_NODES && g_node_id_map[parent_id])
        return g_node_id_map[parent_id];
    return g_window;
}

int64_t haki_gtk_create_label(int64_t parent_id, const char* text) {
    int64_t id = g_next_id++;
    if (id >= HAKI_MAX_NODES) return -1;
    wchar_t* wtext = utf8_to_wide(text ? text : "");
    HWND hw = CreateWindowExW(
        0, L"STATIC", wtext,
        WS_CHILD | WS_VISIBLE | SS_LEFT,
        0, 0, 200, 24,
        get_parent_hwnd(parent_id), (HMENU)(intptr_t)id,
        GetModuleHandleW(NULL), NULL
    );
    free(wtext);
    g_node_id_map[id] = hw;
    return id;
}

int64_t haki_gtk_create_button(int64_t parent_id, const char* label, int64_t node_id_hint) {
    int64_t id = node_id_hint > 0 ? node_id_hint : g_next_id++;
    if (id >= HAKI_MAX_NODES) return -1;
    if (id >= g_next_id) g_next_id = id + 1;
    wchar_t* wlabel = utf8_to_wide(label ? label : "");
    HWND hw = CreateWindowExW(
        0, L"BUTTON", wlabel,
        WS_CHILD | WS_VISIBLE | BS_PUSHBUTTON,
        0, 0, 120, 32,
        get_parent_hwnd(parent_id), (HMENU)(intptr_t)id,
        GetModuleHandleW(NULL), NULL
    );
    free(wlabel);
    g_node_id_map[id] = hw;
    return id;
}

int64_t haki_gtk_create_box(int64_t parent_id, int64_t horizontal) {
    int64_t id = g_next_id++;
    if (id >= HAKI_MAX_NODES) return -1;
    /* Boxes are plain windows that manage layout of their children */
    HWND hw = CreateWindowExW(
        0, L"STATIC", L"",
        WS_CHILD | WS_VISIBLE | SS_LEFT,
        0, 0, 400, 400,
        get_parent_hwnd(parent_id), (HMENU)(intptr_t)id,
        GetModuleHandleW(NULL), NULL
    );
    g_node_id_map[id] = hw;
    g_boxes[id].is_row = (int)horizontal;
    g_boxes[id].child_count = 0;
    g_boxes[id].padding = 8;
    return id;
}

int64_t haki_gtk_create_text_field(int64_t parent_id, const char* placeholder, int64_t node_id) {
    int64_t id = node_id > 0 ? node_id : g_next_id++;
    if (id >= HAKI_MAX_NODES) return -1;
    if (id >= g_next_id) g_next_id = id + 1;
    HWND hw = CreateWindowExW(
        WS_EX_CLIENTEDGE, L"EDIT", L"",
        WS_CHILD | WS_VISIBLE | ES_AUTOHSCROLL,
        0, 0, 200, 24,
        get_parent_hwnd(parent_id), (HMENU)(intptr_t)id,
        GetModuleHandleW(NULL), NULL
    );
    if (placeholder && placeholder[0]) {
        wchar_t* wp = utf8_to_wide(placeholder);
        SendMessageW(hw, EM_SETCUEBANNER, TRUE, (LPARAM)wp);
        free(wp);
    }
    g_node_id_map[id] = hw;
    return id;
}

int64_t haki_gtk_create_checkbox(int64_t parent_id, const char* label, int64_t checked, int64_t node_id) {
    int64_t id = node_id > 0 ? node_id : g_next_id++;
    if (id >= HAKI_MAX_NODES) return -1;
    if (id >= g_next_id) g_next_id = id + 1;
    wchar_t* wlabel = utf8_to_wide(label ? label : "");
    HWND hw = CreateWindowExW(
        0, L"BUTTON", wlabel,
        WS_CHILD | WS_VISIBLE | BS_AUTOCHECKBOX,
        0, 0, 150, 24,
        get_parent_hwnd(parent_id), (HMENU)(intptr_t)id,
        GetModuleHandleW(NULL), NULL
    );
    free(wlabel);
    SendMessageW(hw, BM_SETCHECK, checked ? BST_CHECKED : BST_UNCHECKED, 0);
    g_node_id_map[id] = hw;
    return id;
}

int64_t haki_gtk_create_dropdown(int64_t parent_id, const char* options_csv, int64_t node_id) {
    int64_t id = node_id > 0 ? node_id : g_next_id++;
    if (id >= HAKI_MAX_NODES) return -1;
    if (id >= g_next_id) g_next_id = id + 1;
    HWND hw = CreateWindowExW(
        0, L"COMBOBOX", L"",
        WS_CHILD | WS_VISIBLE | CBS_DROPDOWNLIST | WS_VSCROLL,
        0, 0, 200, 200,
        get_parent_hwnd(parent_id), (HMENU)(intptr_t)id,
        GetModuleHandleW(NULL), NULL
    );
    if (options_csv && options_csv[0]) {
        char* buf = _strdup(options_csv);
        char* tok = strtok(buf, ",");
        while (tok) {
            wchar_t* wopt = utf8_to_wide(tok);
            SendMessageW(hw, CB_ADDSTRING, 0, (LPARAM)wopt);
            free(wopt);
            tok = strtok(NULL, ",");
        }
        free(buf);
        SendMessageW(hw, CB_SETCURSEL, 0, 0);
    }
    g_node_id_map[id] = hw;
    return id;
}

int64_t haki_gtk_create_image(int64_t parent_id, const char* path, int64_t w, int64_t h) {
    int64_t id = g_next_id++;
    if (id >= HAKI_MAX_NODES) return -1;
    wchar_t* wpath = utf8_to_wide(path ? path : "");
    HBITMAP bmp = (HBITMAP)LoadImageW(NULL, wpath, IMAGE_BITMAP, (int)w, (int)h, LR_LOADFROMFILE);
    free(wpath);
    HWND hw = CreateWindowExW(
        0, L"STATIC", L"",
        WS_CHILD | WS_VISIBLE | SS_BITMAP,
        0, 0, (int)w > 0 ? (int)w : 100, (int)h > 0 ? (int)h : 100,
        get_parent_hwnd(parent_id), (HMENU)(intptr_t)id,
        GetModuleHandleW(NULL), NULL
    );
    if (bmp) SendMessageW(hw, STM_SETIMAGE, IMAGE_BITMAP, (LPARAM)bmp);
    g_node_id_map[id] = hw;
    return id;
}

/* ── Mutation API ───────────────────────────────────────────────────────── */

void haki_gtk_set_text(int64_t node_id, const char* text) {
    if (node_id <= 0 || node_id >= HAKI_MAX_NODES || !g_node_id_map[node_id]) return;
    wchar_t* wtext = utf8_to_wide(text ? text : "");
    SetWindowTextW(g_node_id_map[node_id], wtext);
    free(wtext);
}

void haki_gtk_set_visible(int64_t node_id, int64_t visible) {
    if (node_id <= 0 || node_id >= HAKI_MAX_NODES || !g_node_id_map[node_id]) return;
    ShowWindow(g_node_id_map[node_id], visible ? SW_SHOW : SW_HIDE);
}

void haki_gtk_insert_child(int64_t parent_id, int64_t index, int64_t child_id) {
    if (parent_id <= 0 || parent_id >= HAKI_MAX_NODES) return;
    if (child_id  <= 0 || child_id  >= HAKI_MAX_NODES) return;
    HWND parent_hw = g_node_id_map[parent_id];
    HWND child_hw  = g_node_id_map[child_id];
    if (!parent_hw || !child_hw) return;
    SetParent(child_hw, parent_hw);
    /* Track in box layout */
    HakiBox* box = &g_boxes[parent_id];
    int idx = (int)index;
    if (idx < 0 || idx > box->child_count) idx = box->child_count;
    if (box->child_count < 255) {
        memmove(&box->children[idx + 1], &box->children[idx],
                (box->child_count - idx) * sizeof(HWND));
        box->children[idx] = child_hw;
        box->child_count++;
    }
    haki_reflow_box(parent_id);
}

void haki_gtk_remove_child(int64_t node_id) {
    if (node_id <= 0 || node_id >= HAKI_MAX_NODES || !g_node_id_map[node_id]) return;
    DestroyWindow(g_node_id_map[node_id]);
    g_node_id_map[node_id] = NULL;
}

void haki_gtk_register_node(int64_t vnode_id, int64_t gtk_id) {
    if (vnode_id >= 0 && vnode_id < HAKI_MAX_NODES &&
        gtk_id    >= 0 && gtk_id    < HAKI_MAX_NODES) {
        g_node_id_map[vnode_id] = g_node_id_map[gtk_id];
    }
}

void haki_gtk_set_callback(int64_t node_id, int64_t cb_id) {
    (void)node_id; (void)cb_id; /* stable callbacks — no-op for now */
}

/* ── Layout props ───────────────────────────────────────────────────────── */

void haki_gtk_set_padding(int64_t node_id, int64_t px) {
    if (node_id > 0 && node_id < HAKI_MAX_NODES) {
        g_boxes[node_id].padding = (int)px;
        haki_reflow_box(node_id);
    }
}

void haki_gtk_set_spacing(int64_t node_id, int64_t px) {
    haki_gtk_set_padding(node_id, px); /* same concept on Win32 */
}

void haki_gtk_set_alignment(int64_t node_id, const char* align) {
    if (node_id <= 0 || node_id >= HAKI_MAX_NODES || !g_node_id_map[node_id]) return;
    /* Win32: adjust window style for alignment */
    HWND hw = g_node_id_map[node_id];
    LONG_PTR style = GetWindowLongPtrW(hw, GWL_STYLE);
    style &= ~(SS_LEFT | SS_CENTER | SS_RIGHT);
    if (align && strcmp(align, "center") == 0) style |= SS_CENTER;
    else if (align && strcmp(align, "end") == 0) style |= SS_RIGHT;
    else style |= SS_LEFT;
    SetWindowLongPtrW(hw, GWL_STYLE, style);
    InvalidateRect(hw, NULL, TRUE);
}

/* ── Legacy stubs (v3.1 compat) ─────────────────────────────────────────── */

int64_t haki_gtk_alloc_node_id(void) { return g_next_id++; }
int64_t haki_gtk_peek_next_id(void)  { return g_next_id;   }
void    haki_gtk_mark_label(int64_t node_id) { (void)node_id; }
int64_t haki_gtk_get_label_id(void)  { return 0; }
void    haki_set_callback_dispatcher(void* fn) { (void)fn; }
void    haki_fire_callback(int64_t id) { fire_callback(id); }
void*   haki_get_callback(int64_t id) {
    if (id <= 0 || id >= HAKI_MAX_CALLBACKS) return NULL;
    /* Return a fat pointer to the callback */
    static void* fat[2];
    fat[0] = (void*)g_callbacks[id];
    fat[1] = g_callback_envs[id];
    return fat;
}

/* ── Event loop ─────────────────────────────────────────────────────────── */

void haki_platform_run(void) {
    if (!g_window) return;
    ShowWindow(g_window, SW_SHOWDEFAULT);
    UpdateWindow(g_window);
    MSG msg;
    while (GetMessageW(&msg, NULL, 0, 0) > 0) {
        TranslateMessage(&msg);
        DispatchMessageW(&msg);
    }
}

/* ── Legacy haki_app_run (JSON bridge compat) ───────────────────────────── */

void haki_app_run(const char* json, const char* title, int64_t width, int64_t height) {
    (void)json;
    haki_gtk_create_window(title, width, height);
    haki_platform_run();
}
