#include <ges/ges.h>
#include <gst/gst.h>
#include <stdio.h>
#include <string.h>
#include <unistd.h>

static int fail(const char *message) {
  fprintf(stderr, "%s\n", message);
  return 1;
}

static int probe_factory(const char *name) {
  GstElementFactory *factory = gst_element_factory_find(name);
  if (factory == NULL) {
    fprintf(stderr, "required factory is unavailable: %s\n", name);
    return 1;
  }
  GstPlugin *plugin = gst_plugin_feature_get_plugin(GST_PLUGIN_FEATURE(factory));
  if (plugin == NULL) {
    gst_object_unref(factory);
    fprintf(stderr, "required factory has no plugin provenance: %s\n", name);
    return 1;
  }
  printf("FACTORY\t%s\t%s\t%s\t%s\n", name,
         gst_plugin_feature_get_plugin_name(GST_PLUGIN_FEATURE(factory)),
         gst_plugin_get_package(plugin), gst_plugin_get_license(plugin));
  fflush(stdout);
  gst_object_unref(plugin);
  gst_object_unref(factory);
  return 0;
}

static int smoke_ges_timeline(const char *image_path) {
  fprintf(stderr, "GES_SMOKE_CHECKPOINT\turi\n");
  fflush(stderr);
  GError *error = NULL;
  gchar *uri = gst_filename_to_uri(image_path, &error);
  if (uri == NULL) {
    if (error != NULL) {
      fprintf(stderr, "could not create GES smoke URI: %s\n", error->message);
      g_error_free(error);
    }
    return 1;
  }

  GESTimeline *timeline = ges_timeline_new_audio_video();
  GESLayer *layer = timeline == NULL ? NULL : ges_timeline_append_layer(timeline);
  GESUriClip *clip = ges_uri_clip_new(uri);
  g_free(uri);
  if (timeline == NULL || layer == NULL || clip == NULL) {
    if (clip != NULL) gst_object_unref(clip);
    if (timeline != NULL) gst_object_unref(timeline);
    return fail("could not construct the GES smoke timeline");
  }
  fprintf(stderr, "GES_SMOKE_CHECKPOINT\tcommit\n");
  fflush(stderr);
  ges_uri_clip_set_is_image(clip, TRUE);
  if (!ges_timeline_element_set_duration(
          GES_TIMELINE_ELEMENT(clip), 100 * GST_MSECOND) ||
      !ges_layer_add_clip(layer, GES_CLIP(clip)) ||
      !ges_timeline_commit_sync(timeline)) {
    gst_object_unref(timeline);
    return fail("could not commit the GES smoke timeline");
  }

  GESPipeline *pipeline = ges_pipeline_new();
  GstElement *video_sink = gst_element_factory_make("fakesink", NULL);
  GstElement *audio_sink = gst_element_factory_make("fakesink", NULL);
  if (pipeline == NULL || video_sink == NULL || audio_sink == NULL ||
      !ges_pipeline_set_timeline(pipeline, timeline)) {
    if (video_sink != NULL) gst_object_unref(video_sink);
    if (audio_sink != NULL) gst_object_unref(audio_sink);
    if (pipeline != NULL) gst_object_unref(pipeline);
    gst_object_unref(timeline);
    return fail("could not attach the GES smoke timeline");
  }
  ges_pipeline_preview_set_video_sink(pipeline, video_sink);
  ges_pipeline_preview_set_audio_sink(pipeline, audio_sink);
  gst_object_unref(video_sink);
  gst_object_unref(audio_sink);
  fprintf(stderr, "GES_SMOKE_CHECKPOINT\tpipeline\n");
  fflush(stderr);
  if (!ges_pipeline_set_mode(pipeline, GES_PIPELINE_MODE_PREVIEW)) {
    return fail("could not configure the GES smoke timeline");
  }
  printf("GES_SMOKE\tpassed\timagefreeze+gio+nle\n");
  fflush(stdout);
  _exit(0);
}

int main(int argc, char **argv) {
  if (argc < 4 || strcmp(argv[1], "--image") != 0) {
    return fail(
        "usage: gstreamer-runtime-probe --image /path/frame.png FACTORY...");
  }
  gst_init(NULL, NULL);
  if (!ges_init()) return fail("GES initialization failed");
  for (int index = 3; index < argc; index += 1) {
    if (probe_factory(argv[index]) != 0) return 1;
  }
  return smoke_ges_timeline(argv[2]);
}
