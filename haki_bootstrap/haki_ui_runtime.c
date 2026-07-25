
/* haki_ui_runtime.c — GTK 3 backend for haki_ui
   Implements the native widget shim for Text, Button, VStack, HStack,
   Spacer, TextField, and the App event loop.                         */

#include <gtk/gtk.h>
#include <stdlib.h>
#include <string.h>
#include <stdint.h>

GtkWidget* haki_text_new(const char* content) {
    return gtk_label_new(content ? content : "");
}

void haki_text_set(GtkWidget* w, const char* content) {
    if (w) gtk_label_set_text(GTK_LABEL(w), content ? content : "");
}

typedef void (*HakiCallback)(void);
typedef struct { HakiCallback fn; } HakiButtonData;

static void haki_button_clicked(GtkButton* btn, gpointer data) {
    (void)btn;
    HakiButtonData* d = (HakiButtonData*)data;
    if (d && d->fn) d->fn();
}

GtkWidget* haki_button_new(const char* label, HakiCallback on_tap) {
    GtkWidget* btn = gtk_button_new_with_label(label ? label : "");
    if (on_tap) {
        HakiButtonData* data = (HakiButtonData*)malloc(sizeof(HakiButtonData));
        data->fn = on_tap;
        g_signal_connect(btn, "clicked", G_CALLBACK(haki_button_clicked), data);
    }
    return btn;
}

typedef void (*HakiStringCallback)(const char*);
typedef struct { HakiStringCallback fn; } HakiEntryData;

static void haki_entry_changed(GtkEntry* entry, gpointer data) {
    HakiEntryData* d = (HakiEntryData*)data;
    if (d && d->fn) d->fn(gtk_entry_get_text(entry));
}

GtkWidget* haki_textfield_new(const char* value, HakiStringCallback on_change) {
    GtkWidget* entry = gtk_entry_new();
    if (value) gtk_entry_set_text(GTK_ENTRY(entry), value);
    if (on_change) {
        HakiEntryData* data = (HakiEntryData*)malloc(sizeof(HakiEntryData));
        data->fn = on_change;
        g_signal_connect(entry, "changed", G_CALLBACK(haki_entry_changed), data);
    }
    return entry;
}

GtkWidget* haki_vstack_new(GtkWidget** children, int64_t count) {
    GtkWidget* box = gtk_box_new(GTK_ORIENTATION_VERTICAL, 4);
    for (int64_t i = 0; i < count; i++)
        if (children[i]) gtk_box_pack_start(GTK_BOX(box), children[i], FALSE, FALSE, 2);
    return box;
}

GtkWidget* haki_hstack_new(GtkWidget** children, int64_t count) {
    GtkWidget* box = gtk_box_new(GTK_ORIENTATION_HORIZONTAL, 4);
    for (int64_t i = 0; i < count; i++)
        if (children[i]) gtk_box_pack_start(GTK_BOX(box), children[i], FALSE, FALSE, 2);
    return box;
}

GtkWidget* haki_spacer_new(void) {
    GtkWidget* s = gtk_label_new("");
    gtk_widget_set_hexpand(s, TRUE);
    gtk_widget_set_vexpand(s, TRUE);
    return s;
}

typedef GtkWidget* (*HakiBodyFn)(void* self);
typedef struct { GtkWidget* window; GtkWidget* current_root; void* root; HakiBodyFn body_fn; } HakiApp;
static HakiApp* g_haki_app = NULL;

static gboolean haki_app_rerender(gpointer data) {
    (void)data;
    if (!g_haki_app || !g_haki_app->body_fn) return G_SOURCE_REMOVE;
    if (g_haki_app->current_root)
        gtk_container_remove(GTK_CONTAINER(g_haki_app->window), g_haki_app->current_root);
    /* Call body_fn(root) to get the new widget tree */
    g_haki_app->current_root = g_haki_app->body_fn(g_haki_app->root);
    if (g_haki_app->current_root) {
        gtk_container_add(GTK_CONTAINER(g_haki_app->window), g_haki_app->current_root);
        gtk_widget_show_all(g_haki_app->window);
    }
    return G_SOURCE_REMOVE;
}

void haki_ui_request_rerender(void) {
    g_idle_add(haki_app_rerender, NULL);
}

static gboolean on_delete(GtkWidget* w, GdkEvent* e, gpointer d) {
    (void)w; (void)e; (void)d; gtk_main_quit(); return FALSE;
}

/* haki_app_run(title, root, body_fn):
   - root:    pointer to the Haki class instance (the root View)
   - body_fn: TypeName__body(self) → GtkWidget* — produces the widget tree
   The runtime calls body_fn(root) once to build the initial tree, then after
   every GTK event to rebuild it with the updated state.                      */
void haki_app_run(const char* title, void* root, HakiBodyFn body_fn) {
    int argc = 0;
    gtk_init(&argc, NULL);
    g_haki_app = (HakiApp*)calloc(1, sizeof(HakiApp));
    g_haki_app->root    = root;
    g_haki_app->body_fn = body_fn;
    g_haki_app->window  = gtk_window_new(GTK_WINDOW_TOPLEVEL);
    gtk_window_set_title(GTK_WINDOW(g_haki_app->window), title ? title : "Haki App");
    gtk_window_set_default_size(GTK_WINDOW(g_haki_app->window), 400, 300);
    gtk_container_set_border_width(GTK_CONTAINER(g_haki_app->window), 12);
    g_signal_connect(g_haki_app->window, "delete-event", G_CALLBACK(on_delete), NULL);
    /* Initial render */
    g_haki_app->current_root = body_fn(root);
    if (g_haki_app->current_root)
        gtk_container_add(GTK_CONTAINER(g_haki_app->window), g_haki_app->current_root);
    gtk_widget_show_all(g_haki_app->window);
    gtk_main();
    free(g_haki_app); g_haki_app = NULL;
}
