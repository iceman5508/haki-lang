/**
 * haki_ui_gtk.c — GTK 3 platform backend for haki_ui v2.x
 *
 * Virtual Tree + diff architecture. The C layer only ever sees integers (node_id).
 * Haki owns the VNode graph and the callback closures. GTK sees no Haki memory.
 *
 * Haki → C:  haki_gtk_create_*(parent_id, ...) → new node_id
 *            haki_gtk_set_text(node_id, text)
 *            haki_gtk_insert_child(parent_id, index, child_id)
 *            haki_gtk_remove_child(node_id)
 *
 * C → Haki:  trigger_haki_callback(node_id)
 *            (Haki registered haki_set_callback_dispatcher before gtk_main)
 *
 * Build (linked by hakic --target gtk automatically):
 *   gcc -O2 $(pkg-config --cflags gtk+-3.0) haki_ui_gtk.c \
 *       $(pkg-config --libs gtk+-3.0) -o myapp
 */

#include <gtk/gtk.h>
#include <stdio.h>
#include <stdlib.h>
#include <string.h>
#include <stdint.h>

/* Forward declarations */
static void do_rerender(void);  /* defined later, called by on_button_clicked */

/* Haki closure = fat pointer: { fn_ptr(void* env), env_ptr }
   Stored as void*[2]: [0]=fn_ptr, [1]=env_ptr */
#define HAKI_MAX_CALLBACKS 4096
static void* g_callbacks_fwd[HAKI_MAX_CALLBACKS];  /* void* closure fat pointer */

static void haki_fire_callback(int64_t node_id) {
    if (node_id <= 0 || node_id >= HAKI_MAX_CALLBACKS) return;
    void* closure = g_callbacks_fwd[node_id];
    if (!closure) return;
    /* haki_make_closure builds void*[2] = { fn_ptr, env_ptr }
       fn_ptr signature: void fn(void* __env) */
    void** fat = (void**)closure;
    void (*fn_ptr)(void*) = (void(*)(void*))fat[0];
    void* env_ptr = fat[1];
    fn_ptr(env_ptr);
}

/* Payload structs for multi-field enum variants */
typedef struct { void* f0; void* f1; } __PayloadTuple2;
typedef struct { void* f0; void* f1; void* f2; } __PayloadTuple3;

// ── Node registry ─────────────────────────────────────────────────────────────
// Maps integer node_id → GtkWidget*.
// GTK only ever sees GtkWidget* internally; Haki only sees int64_t.
// Max 65536 nodes per window — sufficient for any practical UI.

#define HAKI_MAX_NODES 65536

static GtkWidget* g_nodes[HAKI_MAX_NODES];
static int64_t    g_next_id = 1;   // 0 = root window

// v3.4 widget constructors (text field, checkbox, dropdown, image, layout
// setters) address widgets through this map instead of g_nodes/node_get.
// It was referenced throughout the file but never declared, so any build
// touching those widgets failed to link. Alias it onto g_nodes so both
// families of accessors see the same underlying registry rather than
// silently diverging into two separate node tables.
#define g_node_id_map g_nodes

static GtkWidget* node_get(int64_t id) {
    if (id <= 0 || id >= HAKI_MAX_NODES) return NULL;
    return g_nodes[id];
}

static int64_t node_alloc(GtkWidget* w) {
    int64_t id = g_next_id++;
    if (id >= HAKI_MAX_NODES) { fprintf(stderr, "haki_ui: node limit reached\n"); abort(); }
    g_nodes[id] = w;
    return id;
}

static void node_free(int64_t id) {
    if (id > 0 && id < HAKI_MAX_NODES) g_nodes[id] = NULL;
    // Widget is gone — drop its slot in the callback registry too, so a
    // stale/reused node_id can't fire a closure that belonged to a widget
    // that no longer exists. (Defined inline, not via haki_unregister_callback,
    // to avoid a forward declaration — g_callbacks_fwd is already in scope.)
    if (id > 0 && id < HAKI_MAX_CALLBACKS) g_callbacks_fwd[id] = NULL;
}

// ── Callback dispatcher ───────────────────────────────────────────────────────
// Haki calls haki_set_callback_dispatcher() before gtk_main() to register
// the Haki-side closure lookup function.
// When a GTK button is clicked, g_dispatcher(node_id) calls back into Haki.

typedef void (*HakiDispatchFn)(int64_t node_id);
static HakiDispatchFn g_dispatcher = NULL;

void haki_set_callback_dispatcher(HakiDispatchFn fn) {
    g_dispatcher = fn;
}

// GTK signal handler
static void on_button_clicked(GtkWidget* widget, gpointer user_data) {
    int64_t node_id = (int64_t)(intptr_t)user_data;
    haki_fire_callback(node_id);
    if (g_dispatcher) g_dispatcher(node_id);
    // v3.4: drive the full VNode diff cycle, not the legacy single-label
    // shortcut. do_rerender() only ever updated one hardcoded label id
    // (g_label_node_id) via haki_gtk_mark_label's "first call wins" latch —
    // that's why only the first Text node ever refreshed. haki_trigger_rerender
    // calls back into Haki's App.rerender(), which rebuilds the vtree, diffs
    // it against the previous one, and applies a SetText mutation for every
    // node that changed.
    haki_trigger_rerender();
}

// Alias kept for widgets (checkbox/dropdown) whose signal handlers were
// wired to a name ("haki_button_clicked") that was never defined — those
// signals should drive the same rerender path as button clicks.
#define haki_button_clicked on_button_clicked

// ── Window ────────────────────────────────────────────────────────────────────

static GtkWidget* g_window = NULL;

int64_t haki_gtk_create_window(const char* title, int64_t width, int64_t height) {
    gtk_init(NULL, NULL);
    g_window = gtk_window_new(GTK_WINDOW_TOPLEVEL);
    gtk_window_set_title(GTK_WINDOW(g_window), title);
    gtk_window_set_default_size(GTK_WINDOW(g_window), (int)width, (int)height);
    g_signal_connect(g_window, "destroy", G_CALLBACK(gtk_main_quit), NULL);
    // Window itself is node 0 — not in the registry, referenced directly
    return 0;
}

// ── Widget creation ───────────────────────────────────────────────────────────

int64_t haki_gtk_create_label(int64_t parent_id, const char* text) {
    GtkWidget* label = gtk_label_new(text);
    int64_t id = node_alloc(label);
    // Attach to parent if parent is a container
    GtkWidget* parent = node_get(parent_id);
    if (parent && GTK_IS_BOX(parent)) {
        gtk_box_pack_start(GTK_BOX(parent), label, FALSE, FALSE, 4);
    } else if (g_window && parent_id == 0) {
        // Root container
    }
    gtk_widget_show(label);
    return id;
}

int64_t haki_gtk_create_button(int64_t parent_id, const char* label_text, int64_t node_id_hint) {
    GtkWidget* btn = gtk_button_new_with_label(label_text);
    // Use the hint as the node_id for the callback — it must match the
    // node_id Haki stored in its callback registry.
    int64_t id = (node_id_hint > 0) ? node_id_hint : node_alloc(btn);
    if (node_id_hint > 0) g_nodes[node_id_hint] = btn;
    g_signal_connect(btn, "clicked", G_CALLBACK(on_button_clicked),
                     (gpointer)(intptr_t)id);
    GtkWidget* parent = node_get(parent_id);
    if (parent && GTK_IS_BOX(parent)) {
        gtk_box_pack_start(GTK_BOX(parent), btn, FALSE, FALSE, 4);
    }
    gtk_widget_show(btn);
    return id;
}

// horizontal=1 → GtkHBox, horizontal=0 → GtkVBox
int64_t haki_gtk_create_box(int64_t parent_id, int64_t horizontal) {
    GtkWidget* box = gtk_box_new(
        horizontal ? GTK_ORIENTATION_HORIZONTAL : GTK_ORIENTATION_VERTICAL, 8);
    int64_t id = node_alloc(box);
    GtkWidget* parent = node_get(parent_id);
    if (parent && GTK_IS_BOX(parent)) {
        gtk_box_pack_start(GTK_BOX(parent), box, TRUE, TRUE, 4);
    } else if (parent_id == 0 && g_window) {
        gtk_container_add(GTK_CONTAINER(g_window), box);
    }
    gtk_widget_show(box);
    return id;
}

// ── Surgical mutations ────────────────────────────────────────────────────────
// These are the ONLY functions GTK calls. Haki drives all mutations;
// GTK just executes them on its widget tree.

void haki_gtk_set_text(int64_t node_id, const char* text) {
    GtkWidget* w = node_get(node_id);
    if (GTK_IS_LABEL(w)) {
        gtk_label_set_text(GTK_LABEL(w), text);
        gtk_widget_queue_draw(w);
    }
    if (GTK_IS_BUTTON(w)) gtk_button_set_label(GTK_BUTTON(w), text);
}

void haki_gtk_set_visible(int64_t node_id, int64_t visible) {
    GtkWidget* w = node_get(node_id);
    if (!w) return;
    if (visible) gtk_widget_show(w);
    else         gtk_widget_hide(w);
}

void haki_gtk_insert_child(int64_t parent_id, int64_t index, int64_t child_id) {
    GtkWidget* parent = node_get(parent_id);
    GtkWidget* child  = node_get(child_id);
    if (!parent || !child) return;
    if (GTK_IS_BOX(parent)) {
        gtk_box_pack_start(GTK_BOX(parent), child, FALSE, FALSE, 4);
        // Reorder to the requested index
        gtk_box_reorder_child(GTK_BOX(parent), child, (int)index);
    }
    gtk_widget_show(child);
}

void haki_gtk_remove_child(int64_t node_id) {
    GtkWidget* w = node_get(node_id);
    if (!w) return;
    GtkWidget* parent = gtk_widget_get_parent(w);
    if (parent) gtk_container_remove(GTK_CONTAINER(parent), w);
    node_free(node_id);
}

// ── Callback registry ────────────────────────────────────────────────────────
// Maps node_id → Haki closure function pointer.
// Haki calls haki_register_callback(id, fn_ptr) during mount.
// GTK button-clicked signal calls haki_fire_callback(id).

void haki_register_callback(int64_t node_id, void* closure) {
    if (node_id > 0 && node_id < HAKI_MAX_CALLBACKS) {
        // Overwriting a live slot drops the previous fat pointer with no
        // release — see haki_unregister_callback below and the caller-side
        // note in vnode.haki: this is the rerender-time leak, not just the
        // unmount-time one.
        g_callbacks_fwd[node_id] = closure;
    }
}

// Counterpart to haki_register_callback — drops the raw pointer from the
// registry so a destroyed widget's id can't fire a stale closure. This does
// NOT release/free the Haki-side closure environment: there is no
// haki_release_closure/ARC-decrement primitive in this codebase for us to
// call. It only prevents future dispatch through a freed node_id.
void haki_unregister_callback(int64_t node_id) {
    if (node_id > 0 && node_id < HAKI_MAX_CALLBACKS)
        g_callbacks_fwd[node_id] = NULL;
}

int64_t haki_gtk_alloc_node_id_debug(void) {
    int64_t id = g_next_id++;
    return id;
}

// Allocate a stable node_id (separate from widget node_ids)
// Used for buttons so their id is known before widget creation
int64_t haki_gtk_alloc_node_id(void) {
    int64_t id = g_next_id++;
    return id;
}

/* Peek at the next node_id without allocating it */
int64_t haki_gtk_peek_next_id(void) {
    return g_next_id;
}

/* Mark a node as the primary label for rerender */
static int64_t g_marked_label_id = 0;
void haki_gtk_mark_label(int64_t node_id) {
    if (g_marked_label_id == 0) g_marked_label_id = node_id;
}
int64_t haki_gtk_get_label_id(void) {
    return g_marked_label_id;
}

// ── Re-render support ────────────────────────────────────────────────────────
// After a button click, call the Haki fn()->string closure to get new label text
// then update the label widget via haki_gtk_set_text.
//
// The closure is a fat pointer: void*[2] = { fn_ptr, env_ptr }
// fn_ptr signature: const char* fn(void* env)

typedef const char* (*HakiStrFn)(void*);
static HakiStrFn g_rerender_fn  = NULL;
static void*     g_rerender_env = NULL;
static int64_t   g_label_node_id = 0;

void haki_set_rerender_callback(int64_t label_id, void* closure) {
    g_label_node_id = label_id;
    if (closure) {
        void** fat = (void**)closure;
        g_rerender_fn  = (HakiStrFn)fat[0];
        g_rerender_env = fat[1];
    }
}

// Legacy single-label debug helper — unused now that on_button_clicked
// drives haki_trigger_rerender() instead of do_rerender(), kept only so
// nothing outside this file that still calls do_rerender_debug() breaks.
static void do_rerender_debug(void) {
    (void)g_rerender_fn; (void)g_rerender_env; (void)g_label_node_id;
}

static void do_rerender(void) {
    if (!g_rerender_fn || !g_label_node_id) return;
    const char* new_text = g_rerender_fn(g_rerender_env);
    if (new_text) haki_gtk_set_text(g_label_node_id, new_text);
}

// ── Event loop ────────────────────────────────────────────────────────────────

void haki_platform_run(void) {
    if (g_window) gtk_widget_show_all(g_window);
    gtk_main();
}

// ── haki_app_run — called by App.run() in app.haki ───────────────────────────
// This is the legacy entry point that the old JSON bridge used.
// In the new architecture it's still called by App.run() but immediately
// delegates to the VNode mount sequence which Haki drives.
// The `json` param is ignored — App.run() will call mount/diff directly.

void haki_app_run(const char* json, const char* title, int64_t width, int64_t height) {
    (void)json; // VNode architecture: Haki calls haki_gtk_create_* directly
    haki_gtk_create_window(title, width, height);
    // Haki will call haki_platform_run() after mounting the VNode tree
    // For backwards compat: if no VNode tree is mounted, just run the loop
    haki_platform_run();
}

// ── v3.4: Node ID registry ───────────────────────────────────────────────────
// Maps VNode.id (Haki integer) to GTK widget pointer
// Used by applyMutation to find the widget for a given node_id

#define HAKI_NODE_REGISTRY_SIZE 8192
static GtkWidget* g_node_registry[HAKI_NODE_REGISTRY_SIZE];

void haki_gtk_register_node(int64_t vnode_id, int64_t gtk_id) {
    if (vnode_id >= 0 && vnode_id < HAKI_NODE_REGISTRY_SIZE) {
        // gtk_id is actually the node_id we assigned — map vnode→widget
        if (gtk_id >= 0 && gtk_id < HAKI_MAX_NODES) {
            g_node_registry[vnode_id] = g_node_id_map[gtk_id];
        }
    }
}

// ── v3.4: Diff-engine rerender fn ────────────────────────────────────────────
// The new rerender path: a void fn() closure called on every state change.
// Unlike the old path which only updated one label, this calls the full diff cycle.

typedef void (*HakiVoidFn)(void*);
static HakiVoidFn g_rerender_void_fn  = NULL;
static void*      g_rerender_void_env = NULL;

void haki_set_rerender_fn(void* closure) {
    if (closure) {
        void** fat = (void**)closure;
        g_rerender_void_fn  = (HakiVoidFn)fat[0];
        g_rerender_void_env = fat[1];
    }
}

static gboolean haki_idle_rerender(gpointer data) {
    (void)data;
    if (g_rerender_void_fn) {
        g_rerender_void_fn(g_rerender_void_env);
    }
    return G_SOURCE_REMOVE;  // run once
}

// Called from Haki State.set() path via button click handler
void haki_trigger_rerender(void) {
    if (g_rerender_void_fn) {
        g_idle_add(haki_idle_rerender, NULL);
    }
}

// ── v3.4: set_callback (for diff mutations) ───────────────────────────────────
void haki_gtk_set_callback(int64_t node_id, int64_t cb_id) {
    (void)node_id; (void)cb_id;
    // Callback re-registration — for now a no-op as callbacks are stable
    // Full implementation in v3.5 with callback registry refresh
}

// ── v3.4: New widgets ─────────────────────────────────────────────────────────

// TextField — single-line text input
int64_t haki_gtk_create_text_field(int64_t parent_id, const char* placeholder, int64_t node_id) {
    int64_t id = haki_gtk_alloc_node_id();
    if (id >= HAKI_MAX_NODES) return -1;
    GtkWidget* entry = gtk_entry_new();
    if (placeholder && placeholder[0]) {
        gtk_entry_set_placeholder_text(GTK_ENTRY(entry), placeholder);
    }
    g_node_id_map[id] = entry;
    if (parent_id > 0 && parent_id < HAKI_MAX_NODES && g_node_id_map[parent_id]) {
        GtkWidget* parent = g_node_id_map[parent_id];
        if (GTK_IS_CONTAINER(parent)) {
            gtk_container_add(GTK_CONTAINER(parent), entry);
        }
    } else if (g_window) {
        gtk_container_add(GTK_CONTAINER(g_window), entry);
    }
    gtk_widget_show(entry);
    return id;
}

// Checkbox — toggle with label
int64_t haki_gtk_create_checkbox(int64_t parent_id, const char* label, int64_t checked, int64_t node_id) {
    int64_t id = haki_gtk_alloc_node_id();
    if (id >= HAKI_MAX_NODES) return -1;
    GtkWidget* cb = gtk_check_button_new_with_label(label ? label : "");
    gtk_toggle_button_set_active(GTK_TOGGLE_BUTTON(cb), checked ? TRUE : FALSE);
    g_node_id_map[id] = cb;
    // Connect toggled signal to callback dispatcher
    g_signal_connect(cb, "toggled", G_CALLBACK(haki_button_clicked), (gpointer)(intptr_t)node_id);
    if (parent_id > 0 && parent_id < HAKI_MAX_NODES && g_node_id_map[parent_id]) {
        GtkWidget* parent = g_node_id_map[parent_id];
        if (GTK_IS_CONTAINER(parent)) {
            gtk_container_add(GTK_CONTAINER(parent), cb);
        }
    }
    gtk_widget_show(cb);
    return id;
}

// Dropdown — combo box
int64_t haki_gtk_create_dropdown(int64_t parent_id, const char* options_csv, int64_t node_id) {
    int64_t id = haki_gtk_alloc_node_id();
    if (id >= HAKI_MAX_NODES) return -1;
    GtkWidget* combo = gtk_combo_box_text_new();
    // options_csv is comma-separated list of option strings
    if (options_csv && options_csv[0]) {
        char* buf = strdup(options_csv);
        char* tok = strtok(buf, ",");
        while (tok) {
            gtk_combo_box_text_append_text(GTK_COMBO_BOX_TEXT(combo), tok);
            tok = strtok(NULL, ",");
        }
        free(buf);
    }
    gtk_combo_box_set_active(GTK_COMBO_BOX(combo), 0);
    g_node_id_map[id] = combo;
    g_signal_connect(combo, "changed", G_CALLBACK(haki_button_clicked), (gpointer)(intptr_t)node_id);
    if (parent_id > 0 && parent_id < HAKI_MAX_NODES && g_node_id_map[parent_id]) {
        GtkWidget* parent = g_node_id_map[parent_id];
        if (GTK_IS_CONTAINER(parent)) {
            gtk_container_add(GTK_CONTAINER(parent), combo);
        }
    }
    gtk_widget_show(combo);
    return id;
}

// Image — GtkImage from file path
int64_t haki_gtk_create_image(int64_t parent_id, const char* path, int64_t w, int64_t h) {
    int64_t id = haki_gtk_alloc_node_id();
    if (id >= HAKI_MAX_NODES) return -1;
    GtkWidget* img;
    if (w > 0 && h > 0) {
        GdkPixbuf* pb = gdk_pixbuf_new_from_file_at_scale(path, (int)w, (int)h, TRUE, NULL);
        img = pb ? gtk_image_new_from_pixbuf(pb) : gtk_image_new_from_file(path);
    } else {
        img = gtk_image_new_from_file(path);
    }
    g_node_id_map[id] = img;
    if (parent_id > 0 && parent_id < HAKI_MAX_NODES && g_node_id_map[parent_id]) {
        GtkWidget* parent = g_node_id_map[parent_id];
        if (GTK_IS_CONTAINER(parent)) {
            gtk_container_add(GTK_CONTAINER(parent), img);
        }
    }
    gtk_widget_show(img);
    return id;
}

// ── v3.4: Layout ─────────────────────────────────────────────────────────────

// Set padding on a widget (via GTK margin properties)
void haki_gtk_set_padding(int64_t node_id, int64_t px) {
    if (node_id < 0 || node_id >= HAKI_MAX_NODES || !g_node_id_map[node_id]) return;
    GtkWidget* w = g_node_id_map[node_id];
    gtk_widget_set_margin_start(w, (gint)px);
    gtk_widget_set_margin_end(w, (gint)px);
    gtk_widget_set_margin_top(w, (gint)px);
    gtk_widget_set_margin_bottom(w, (gint)px);
}

// Set spacing on a GtkBox
void haki_gtk_set_spacing(int64_t node_id, int64_t px) {
    if (node_id < 0 || node_id >= HAKI_MAX_NODES || !g_node_id_map[node_id]) return;
    GtkWidget* w = g_node_id_map[node_id];
    if (GTK_IS_BOX(w)) gtk_box_set_spacing(GTK_BOX(w), (gint)px);
}

// Set alignment on a widget
void haki_gtk_set_alignment(int64_t node_id, const char* align) {
    if (node_id < 0 || node_id >= HAKI_MAX_NODES || !g_node_id_map[node_id]) return;
    GtkWidget* w = g_node_id_map[node_id];
    GtkAlign a = GTK_ALIGN_START;
    if (align) {
        if (strcmp(align, "center") == 0) a = GTK_ALIGN_CENTER;
        else if (strcmp(align, "end") == 0) a = GTK_ALIGN_END;
        else if (strcmp(align, "fill") == 0) a = GTK_ALIGN_FILL;
    }
    gtk_widget_set_halign(w, a);
    gtk_widget_set_valign(w, a);
}
