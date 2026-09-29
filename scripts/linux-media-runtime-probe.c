/*
 * Build-time and verification probe for the Linux media runtime.
 *
 * The caller controls the plugin set through GST_PLUGIN_SYSTEM_PATH_1_0,
 * GST_PLUGIN_PATH_1_0, GST_PLUGIN_SCANNER and GST_REGISTRY_1_0. Output is
 * line-oriented and tab-separated so the Node builder can parse it without a
 * JSON dependency in C.
 *
 *   linux-media-runtime-probe inventory
 *   linux-media-runtime-probe factories NAME...
 *   linux-media-runtime-probe run PIPELINE [TIMEOUT_SECONDS]
 *   linux-media-runtime-probe decode FILE [TIMEOUT_SECONDS]
 */
#include <gst/gst.h>
#include <stdio.h>
#include <stdlib.h>
#include <string.h>

static void print_field(const char *value) {
  if (value == NULL) return;
  for (const char *cursor = value; *cursor != '\0'; cursor += 1) {
    char c = *cursor;
    putchar(c == '\t' || c == '\n' || c == '\r' ? ' ' : c);
  }
}

static void print_row(const char *tag, const char **fields, int count) {
  fputs(tag, stdout);
  for (int index = 0; index < count; index += 1) {
    putchar('\t');
    print_field(fields[index]);
  }
  putchar('\n');
}

static const char *feature_kind(GstPluginFeature *feature) {
  if (GST_IS_ELEMENT_FACTORY(feature)) return "element";
  if (GST_IS_TYPE_FIND_FACTORY(feature)) return "typefind";
  if (GST_IS_DEVICE_PROVIDER_FACTORY(feature)) return "device-provider";
  if (GST_IS_TRACER_FACTORY(feature)) return "tracer";
  if (GST_IS_DYNAMIC_TYPE_FACTORY(feature)) return "dynamic-type";
  return "other";
}

static int inventory(void) {
  GstRegistry *registry = gst_registry_get();
  GList *plugins = gst_registry_get_plugin_list(registry);
  for (GList *item = plugins; item != NULL; item = item->next) {
    GstPlugin *plugin = GST_PLUGIN(item->data);
    const char *name = gst_plugin_get_name(plugin);
    gboolean blacklisted =
        GST_OBJECT_FLAG_IS_SET(plugin, GST_PLUGIN_FLAG_BLACKLISTED);
    const char *fields[] = {
        name,
        gst_plugin_get_filename(plugin),
        gst_plugin_get_license(plugin),
        gst_plugin_get_package(plugin),
        gst_plugin_get_source(plugin),
        gst_plugin_get_origin(plugin),
        gst_plugin_get_version(plugin),
        blacklisted ? "blacklisted" : "loaded",
    };
    print_row("PLUGIN", fields, 8);
    GList *features = gst_registry_get_feature_list_by_plugin(registry, name);
    for (GList *entry = features; entry != NULL; entry = entry->next) {
      GstPluginFeature *feature = GST_PLUGIN_FEATURE(entry->data);
      char rank[32];
      snprintf(rank, sizeof rank, "%u", gst_plugin_feature_get_rank(feature));
      const char *klass = "";
      if (GST_IS_ELEMENT_FACTORY(feature)) {
        klass = gst_element_factory_get_metadata(
            GST_ELEMENT_FACTORY(feature), GST_ELEMENT_METADATA_KLASS);
      }
      const char *feature_fields[] = {
          name, feature_kind(feature), gst_plugin_feature_get_name(feature),
          rank, klass,
      };
      print_row("FEATURE", feature_fields, 5);
    }
    gst_plugin_feature_list_free(features);
  }
  gst_plugin_list_free(plugins);
  fflush(stdout);
  return 0;
}

static int factories(int argc, char **argv) {
  int missing = 0;
  for (int index = 0; index < argc; index += 1) {
    GstElementFactory *factory = gst_element_factory_find(argv[index]);
    if (factory == NULL) {
      const char *fields[] = {argv[index]};
      print_row("MISSING", fields, 1);
      missing += 1;
      continue;
    }
    GstPlugin *plugin = gst_plugin_feature_get_plugin(GST_PLUGIN_FEATURE(factory));
    const char *fields[] = {
        argv[index],
        gst_plugin_feature_get_plugin_name(GST_PLUGIN_FEATURE(factory)),
        plugin ? gst_plugin_get_package(plugin) : "",
        plugin ? gst_plugin_get_license(plugin) : "",
        plugin ? gst_plugin_get_filename(plugin) : "",
    };
    print_row("FACTORY", fields, 5);
    if (plugin != NULL) gst_object_unref(plugin);
    gst_object_unref(factory);
  }
  fflush(stdout);
  return missing == 0 ? 0 : 3;
}

static void on_deep_element_added(GstBin *bin, GstBin *sub_bin,
                                  GstElement *element, gpointer user_data) {
  (void)bin;
  (void)sub_bin;
  (void)user_data;
  GstElementFactory *factory = gst_element_get_factory(element);
  if (factory == NULL) return;
  const char *klass = gst_element_factory_get_metadata(
      factory, GST_ELEMENT_METADATA_KLASS);
  const char *fields[] = {
      gst_plugin_feature_get_name(GST_PLUGIN_FEATURE(factory)),
      klass ? klass : "",
  };
  print_row("ELEMENT", fields, 2);
  fflush(stdout);
}

static int run_until_eos(GstElement *pipeline, int timeout_seconds) {
  if (gst_element_set_state(pipeline, GST_STATE_PLAYING) ==
      GST_STATE_CHANGE_FAILURE) {
    fprintf(stderr, "pipeline failed to start\n");
    return 2;
  }
  GstBus *bus = gst_element_get_bus(pipeline);
  GstMessage *message = gst_bus_timed_pop_filtered(
      bus, (GstClockTime)timeout_seconds * GST_SECOND,
      GST_MESSAGE_EOS | GST_MESSAGE_ERROR);
  int status = 0;
  if (message == NULL) {
    fprintf(stderr, "pipeline timed out after %d seconds\n", timeout_seconds);
    status = 4;
  } else if (GST_MESSAGE_TYPE(message) == GST_MESSAGE_ERROR) {
    GError *error = NULL;
    gchar *debug = NULL;
    gst_message_parse_error(message, &error, &debug);
    fprintf(stderr, "pipeline error from %s: %s\n%s\n",
            GST_OBJECT_NAME(message->src), error ? error->message : "",
            debug ? debug : "");
    g_clear_error(&error);
    g_free(debug);
    status = 5;
  } else {
    const char *fields[] = {"eos"};
    print_row("RESULT", fields, 1);
  }
  if (message != NULL) gst_message_unref(message);
  gst_object_unref(bus);
  gst_element_set_state(pipeline, GST_STATE_NULL);
  fflush(stdout);
  return status;
}

/* Reports mapped shared objects whose path mentions libav so callers can prove
 * which FFmpeg build was actually loaded. */
static void print_loaded_libav_libraries(void) {
  FILE *maps = fopen("/proc/self/maps", "r");
  if (maps == NULL) return;
  GHashTable *seen = g_hash_table_new_full(g_str_hash, g_str_equal, g_free, NULL);
  char line[4096];
  while (fgets(line, sizeof line, maps) != NULL) {
    char *path = strchr(line, '/');
    if (path == NULL) continue;
    path[strcspn(path, "\n")] = '\0';
    if (strstr(path, "libav") == NULL && strstr(path, "libswresample") == NULL &&
        strstr(path, "libswscale") == NULL && strstr(path, "libpostproc") == NULL) {
      continue;
    }
    if (g_hash_table_contains(seen, path)) continue;
    g_hash_table_add(seen, g_strdup(path));
    const char *fields[] = {path};
    print_row("LOADED", fields, 1);
  }
  g_hash_table_destroy(seen);
  fclose(maps);
  fflush(stdout);
}

static int run_pipeline(const char *description, int timeout_seconds) {
  GError *error = NULL;
  GstElement *pipeline = gst_parse_launch(description, &error);
  if (pipeline == NULL || error != NULL) {
    fprintf(stderr, "pipeline parse failed: %s\n",
            error ? error->message : "unknown error");
    g_clear_error(&error);
    if (pipeline != NULL) gst_object_unref(pipeline);
    return 2;
  }
  if (GST_IS_BIN(pipeline)) {
    g_signal_connect(pipeline, "deep-element-added",
                     G_CALLBACK(on_deep_element_added), NULL);
  }
  int status = run_until_eos(pipeline, timeout_seconds);
  print_loaded_libav_libraries();
  gst_object_unref(pipeline);
  return status;
}

static int decode_file(const char *path, int timeout_seconds) {
  GError *error = NULL;
  gchar *uri = gst_filename_to_uri(path, &error);
  if (uri == NULL) {
    fprintf(stderr, "invalid path: %s\n", error ? error->message : path);
    g_clear_error(&error);
    return 2;
  }
  gchar *description = g_strdup_printf(
      "uridecodebin uri=\"%s\" name=d "
      "d. ! queue ! videoconvert ! video/x-raw ! fakesink sync=false "
      "d. ! queue ! audioconvert ! audio/x-raw ! fakesink sync=false",
      uri);
  g_free(uri);
  int status = run_pipeline(description, timeout_seconds);
  g_free(description);
  return status;
}

int main(int argc, char **argv) {
  gst_init(NULL, NULL);
  if (argc >= 2 && strcmp(argv[1], "inventory") == 0) return inventory();
  if (argc >= 3 && strcmp(argv[1], "factories") == 0) {
    return factories(argc - 2, argv + 2);
  }
  if (argc >= 3 && strcmp(argv[1], "run") == 0) {
    return run_pipeline(argv[2], argc >= 4 ? atoi(argv[3]) : 60);
  }
  if (argc >= 3 && strcmp(argv[1], "decode") == 0) {
    return decode_file(argv[2], argc >= 4 ? atoi(argv[3]) : 60);
  }
  fprintf(stderr,
          "usage: linux-media-runtime-probe inventory | factories NAME... | "
          "run PIPELINE [TIMEOUT] | decode FILE [TIMEOUT]\n");
  return 1;
}
