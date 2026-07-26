/**
 * haki_ui_gtk.c — GTK 3 platform backend for haki_ui.
 *
 * Implements the three platform functions called by haki_ui/app.haki:
 *   haki_ui_init(title, width, height)  — create the GTK window
 *   haki_ui_render(element_json)        — build GTK widget tree from Element JSON
 *   haki_ui_run_loop()                  — enter gtk_main()
 *
 * Build (linked by hakic --target gtk automatically):
 *   gcc -O2 $(pkg-config --cflags gtk+-3.0) haki_ui_gtk.c \
 *       $(pkg-config --libs gtk+-3.0) -o myapp
 *
 * The JSON element tree is parsed with a minimal recursive descent parser —
 * no external JSON library needed. The format is produced by element_to_json()
 * in haki_ui/app.haki and is well-constrained.
 */

#include <gtk/gtk.h>
#include <stdio.h>
#include <stdlib.h>
#include <string.h>
#include <stdint.h>

// ── Platform state ─────────────────────────────────────────────────────────────

static GtkWidget* g_window    = NULL;
static GtkWidget* g_root_box  = NULL;
static int        g_width     = 800;
static int        g_height    = 600;

// ── JSON parser (minimal, no alloc) ───────────────────────────────────────────

typedef struct {
    const char* src;
    int         pos;
} JsonParser;

static void jp_skip_ws(JsonParser* p) {
    while (p->src[p->pos] == ' ' || p->src[p->pos] == '\n' ||
           p->src[p->pos] == '\r' || p->src[p->pos] == '\t') p->pos++;
}

static char* jp_string(JsonParser* p) {
    jp_skip_ws(p);
    if (p->src[p->pos] != '"') return NULL;
    p->pos++; // skip opening quote
    int start = p->pos;
    while (p->src[p->pos] && p->src[p->pos] != '"') p->pos++;
    int len = p->pos - start;
    char* s = (char*)malloc(len + 1);
    memcpy(s, p->src + start, len);
    s[len] = 0;
    p->pos++; // skip closing quote
    return s;
}

static int jp_int(JsonParser* p) {
    jp_skip_ws(p);
    int neg = (p->src[p->pos] == '-');
    if (neg) p->pos++;
    int v = 0;
    while (p->src[p->pos] >= '0' && p->src[p->pos] <= '9') {
        v = v * 10 + (p->src[p->pos++] - '0');
    }
    return neg ? -v : v;
}

static int jp_bool(JsonParser* p) {
    jp_skip_ws(p);
    if (strncmp(p->src + p->pos, "true", 4) == 0) { p->pos += 4; return 1; }
    if (strncmp(p->src + p->pos, "false", 5) == 0) { p->pos += 5; return 0; }
    return 0;
}

// Skip to matching }, tracking depth
static void jp_skip_object(JsonParser* p) {
    jp_skip_ws(p);
    if (p->src[p->pos] != '{') return;
    int depth = 0;
    while (p->src[p->pos]) {
        if (p->src[p->pos] == '{') depth++;
        else if (p->src[p->pos] == '}') { depth--; p->pos++; if (!depth) return; continue; }
        else if (p->src[p->pos] == '"') { jp_string(p); continue; }
        p->pos++;
    }
}

static char* jp_get_string_field(JsonParser* p, const char* field) {
    const char* s = p->src + p->pos;
    char needle[128];
    snprintf(needle, sizeof(needle), "\"%s\":", field);
    const char* found = strstr(s, needle);
    if (!found) return NULL;
    JsonParser tmp;
    tmp.src = found + strlen(needle);
    tmp.pos = 0;
    return jp_string(&tmp);
}

static int jp_get_int_field(JsonParser* p, const char* field) {
    const char* s = p->src + p->pos;
    char needle[128];
    snprintf(needle, sizeof(needle), "\"%s\":", field);
    const char* found = strstr(s, needle);
    if (!found) return 0;
    JsonParser tmp;
    tmp.src = found + strlen(needle);
    tmp.pos = 0;
    return jp_int(&tmp);
}

// ── Style application ─────────────────────────────────────────────────────────

static void apply_style(GtkWidget* widget, JsonParser* p) {
    char* color = jp_get_string_field(p, "color");
    int font_size = jp_get_int_field(p, "fontSize");

    if ((color && color[0]) || font_size > 0) {
        GtkCssProvider* css = gtk_css_provider_new();
        char css_buf[512] = "";
        if (color && color[0])
            snprintf(css_buf + strlen(css_buf), sizeof(css_buf) - strlen(css_buf),
                     "* { color: %s; }", color);
        if (font_size > 0)
            snprintf(css_buf + strlen(css_buf), sizeof(css_buf) - strlen(css_buf),
                     "* { font-size: %dpt; }", font_size);
        gtk_css_provider_load_from_data(css, css_buf, -1, NULL);
        gtk_style_context_add_provider(
            gtk_widget_get_style_context(widget),
            GTK_STYLE_PROVIDER(css),
            GTK_STYLE_PROVIDER_PRIORITY_APPLICATION);
        g_object_unref(css);
    }
    free(color);
}

// ── Element → GTK widget ──────────────────────────────────────────────────────

static GtkWidget* element_to_widget(const char* json);

// Handler data passed to GTK signal callbacks
typedef struct {
    void* haki_closure;   /* Haki closure fat pointer (fn_ptr, env) */
} HandlerData;

static void on_button_click(GtkWidget* w, gpointer data) {
    (void)w;
    HandlerData* hd = (HandlerData*)data;
    if (!hd || !hd->haki_closure) return;
    // Call the Haki fn() -> void closure
    // The closure is a fat pointer: [fn_ptr | env_ptr]
    // fn_ptr signature: void(*)(void* env)
    void** fat = (void**)hd->haki_closure;
    void (*fn_ptr)(void*) = (void(*)(void*))fat[0];
    void*  env_ptr        = fat[1];
    fn_ptr(env_ptr);
}

static GtkWidget* build_column(const char* json) {
    GtkWidget* box = gtk_box_new(GTK_ORIENTATION_VERTICAL, 0);

    // Extract spacing
    JsonParser p = { json, 0 };
    int spacing = jp_get_int_field(&p, "spacing");
    gtk_box_set_spacing(GTK_BOX(box), spacing > 0 ? spacing : 8);

    // Find children array
    const char* children_start = strstr(json, "\"children\":[");
    if (children_start) {
        children_start += strlen("\"children\":[");
        // Walk children
        const char* cur = children_start;
        while (*cur && *cur != ']') {
            if (*cur == '{') {
                // Find end of this child object
                int depth = 0;
                const char* start = cur;
                while (*cur) {
                    if (*cur == '{') depth++;
                    else if (*cur == '}') { depth--; if (!depth) { cur++; break; } }
                    cur++;
                }
                // Build sub-widget for this child
                int child_len = (int)(cur - start);
                char* child_json = (char*)malloc(child_len + 1);
                memcpy(child_json, start, child_len);
                child_json[child_len] = 0;
                GtkWidget* child_widget = element_to_widget(child_json);
                free(child_json);
                if (child_widget) {
                    gtk_box_pack_start(GTK_BOX(box), child_widget, FALSE, FALSE, 0);
                }
                if (*cur == ',') cur++; // skip comma
            } else {
                cur++;
            }
        }
    }
    return box;
}

static GtkWidget* build_row(const char* json) {
    GtkWidget* box = gtk_box_new(GTK_ORIENTATION_HORIZONTAL, 0);
    JsonParser p = { json, 0 };
    int spacing = jp_get_int_field(&p, "spacing");
    gtk_box_set_spacing(GTK_BOX(box), spacing > 0 ? spacing : 8);

    const char* children_start = strstr(json, "\"children\":[");
    if (children_start) {
        children_start += strlen("\"children\":[");
        const char* cur = children_start;
        while (*cur && *cur != ']') {
            if (*cur == '{') {
                int depth = 0;
                const char* start = cur;
                while (*cur) {
                    if (*cur == '{') depth++;
                    else if (*cur == '}') { depth--; if (!depth) { cur++; break; } }
                    cur++;
                }
                int child_len = (int)(cur - start);
                char* child_json = (char*)malloc(child_len + 1);
                memcpy(child_json, start, child_len);
                child_json[child_len] = 0;
                GtkWidget* child_widget = element_to_widget(child_json);
                free(child_json);
                if (child_widget)
                    gtk_box_pack_start(GTK_BOX(box), child_widget, FALSE, FALSE, 0);
                if (*cur == ',') cur++;
            } else {
                cur++;
            }
        }
    }
    return box;
}

static GtkWidget* element_to_widget(const char* json) {
    JsonParser p = { json, 0 };
    char* type = jp_get_string_field(&p, "type");
    if (!type) return gtk_label_new("?");

    GtkWidget* w = NULL;

    if (strcmp(type, "Empty") == 0) {
        w = gtk_label_new(""); // invisible placeholder

    } else if (strcmp(type, "Text") == 0 ||
               strcmp(type, "Paragraph") == 0) {
        char* value = jp_get_string_field(&p, "value");
        w = gtk_label_new(value ? value : "");
        gtk_label_set_line_wrap(GTK_LABEL(w), TRUE);
        gtk_widget_set_halign(w, GTK_ALIGN_START);
        free(value);

    } else if (strcmp(type, "Heading") == 0) {
        char* value = jp_get_string_field(&p, "value");
        int level = jp_get_int_field(&p, "level");
        char markup[1024];
        const char* sizes[] = {"xx-large","x-large","large","medium","small","x-small"};
        const char* sz = sizes[(level >= 1 && level <= 6) ? (level - 1) : 0];
        snprintf(markup, sizeof(markup), "<span size=\"%s\" weight=\"bold\">%s</span>",
                 sz, value ? value : "");
        w = gtk_label_new(NULL);
        gtk_label_set_markup(GTK_LABEL(w), markup);
        gtk_widget_set_halign(w, GTK_ALIGN_START);
        free(value);

    } else if (strcmp(type, "Button") == 0) {
        char* label = jp_get_string_field(&p, "label");
        w = gtk_button_new_with_label(label ? label : "");
        // onClick will be wired by the event handler registration system
        free(label);

    } else if (strcmp(type, "TextInput") == 0) {
        char* placeholder = jp_get_string_field(&p, "placeholder");
        char* value       = jp_get_string_field(&p, "value");
        w = gtk_entry_new();
        if (placeholder && placeholder[0])
            gtk_entry_set_placeholder_text(GTK_ENTRY(w), placeholder);
        if (value && value[0])
            gtk_entry_set_text(GTK_ENTRY(w), value);
        free(placeholder);
        free(value);

    } else if (strcmp(type, "Checkbox") == 0) {
        char* label   = jp_get_string_field(&p, "label");
        int   checked = jp_get_int_field(&p, "checked");
        w = gtk_check_button_new_with_label(label ? label : "");
        gtk_toggle_button_set_active(GTK_TOGGLE_BUTTON(w), checked);
        free(label);

    } else if (strcmp(type, "Column") == 0) {
        w = build_column(json);

    } else if (strcmp(type, "Row") == 0) {
        w = build_row(json);

    } else if (strcmp(type, "Spacer") == 0) {
        w = gtk_label_new("");
        gtk_widget_set_hexpand(w, TRUE);
        gtk_widget_set_vexpand(w, TRUE);

    } else if (strcmp(type, "Image") == 0) {
        char* src = jp_get_string_field(&p, "src");
        w = gtk_image_new_from_file(src ? src : "");
        free(src);

    } else if (strcmp(type, "ScrollView") == 0) {
        w = gtk_scrolled_window_new(NULL, NULL);
        const char* child_start = strstr(json, "\"child\":{");
        if (child_start) {
            child_start += strlen("\"child\":");
            int depth = 0;
            const char* cur = child_start;
            while (*cur) {
                if (*cur == '{') depth++;
                else if (*cur == '}') { depth--; if (!depth) { cur++; break; } }
                cur++;
            }
            int len = (int)(cur - child_start);
            char* child_json = (char*)malloc(len + 1);
            memcpy(child_json, child_start, len);
            child_json[len] = 0;
            GtkWidget* child = element_to_widget(child_json);
            free(child_json);
            if (child) gtk_container_add(GTK_CONTAINER(w), child);
        }

    } else {
        w = gtk_label_new(type); // unknown — show type name for debugging
    }

    free(type);
    if (w) gtk_widget_show(w);
    return w;
}

// ── Platform API ───────────────────────────────────────────────────────────────

int64_t haki_ui_init(const char* title, int64_t width, int64_t height) {
    g_width  = (int)width;
    g_height = (int)height;

    int argc = 0;
    gtk_init(&argc, NULL);

    g_window = gtk_window_new(GTK_WINDOW_TOPLEVEL);
    gtk_window_set_title(GTK_WINDOW(g_window), title ? title : "Haki App");
    gtk_window_set_default_size(GTK_WINDOW(g_window),
                                g_width  > 0 ? g_width  : 800,
                                g_height > 0 ? g_height : 600);
    g_signal_connect(g_window, "destroy", G_CALLBACK(gtk_main_quit), NULL);

    // Root container
    g_root_box = gtk_box_new(GTK_ORIENTATION_VERTICAL, 0);
    gtk_container_add(GTK_CONTAINER(g_window), g_root_box);
    gtk_widget_show(g_root_box);
    gtk_widget_show(g_window);

    return 0;
}

int64_t haki_ui_render(const char* element_json) {
    if (!g_root_box || !element_json) return -1;

    // Remove existing children
    GList* children = gtk_container_get_children(GTK_CONTAINER(g_root_box));
    for (GList* l = children; l != NULL; l = l->next) {
        gtk_widget_destroy(GTK_WIDGET(l->data));
    }
    g_list_free(children);

    // Build new widget tree from JSON
    GtkWidget* new_tree = element_to_widget(element_json);
    if (new_tree) {
        gtk_box_pack_start(GTK_BOX(g_root_box), new_tree, TRUE, TRUE, 0);
    }

    gtk_widget_show_all(g_window);
    return 0;
}

int64_t haki_ui_run_loop(void) {
    gtk_main();  /* blocks until gtk_main_quit() is called */
    return 0;
}
