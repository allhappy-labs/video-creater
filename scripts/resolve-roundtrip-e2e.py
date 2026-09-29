#!/usr/bin/env python3
"""Run Video Creater's FCPXML round-trip inside DaVinci Resolve.

Install or symlink this file into Resolve's Workspace > Scripts menu, then put
the request at ``output/nle-roundtrip-evidence/request.json``. The script uses
only a uniquely named disposable project and always writes a JSON result,
including cleanup and traceback details when a Resolve API call fails.

Minimal request: ``{"fcpxmlPath": "/absolute/edit.fcpxml", "mediaPath":
"/absolute/media-directory"}``. Use ``mediaPaths`` plus
``useSourceClipsFolders: true`` to pre-import exact files and have FCPXML match
against that Media Pool folder. Optional keys include ``timelineName``,
``evidenceDir``, ``resultPath``, ``renderOutputDir``, ``roundtripFcpxmlPath``,
``renderFormat``, ``renderCodec``, ``renderSettings``, and
``renderTimeoutSeconds``.
"""

import datetime
import json
import os
from pathlib import Path
import re
import sys
import time
import traceback
import uuid


REPO_ROOT = Path("/Users/olhapi/Documents/video-creater")
DEFAULT_REQUEST_PATH = REPO_ROOT / "output/nle-roundtrip-evidence/request.json"
PROJECT_PREFIX = "VideoCreater_NLE_RoundTrip_"


def utc_now():
    return datetime.datetime.now(datetime.timezone.utc).isoformat()


def json_safe(value, depth=0):
    """Convert Resolve proxy return values into bounded JSON-safe values."""
    if depth > 8:
        return repr(value)
    if value is None or isinstance(value, (bool, int, float, str)):
        return value
    if isinstance(value, bytes):
        return value.decode("utf-8", errors="replace")
    if isinstance(value, dict):
        return {str(key): json_safe(item, depth + 1) for key, item in value.items()}
    if isinstance(value, (list, tuple, set)):
        return [json_safe(item, depth + 1) for item in value]
    return repr(value)


def write_json(path, payload):
    path = Path(path)
    path.parent.mkdir(parents=True, exist_ok=True)
    temporary = path.with_name(path.name + ".tmp")
    with temporary.open("w", encoding="utf-8") as handle:
        json.dump(json_safe(payload), handle, indent=2, sort_keys=True)
        handle.write("\n")
    os.replace(str(temporary), str(path))


def require(condition, message):
    if not condition:
        raise RuntimeError(message)
    return condition


def call_optional(target, method_name, *args):
    method = getattr(target, method_name, None)
    if not callable(method):
        return None
    try:
        return method(*args)
    except Exception as error:  # Preserve evidence even across Resolve versions.
        return {"error": "%s: %s" % (type(error).__name__, error)}


def resolve_request_path():
    override = os.environ.get("VIDEO_CREATER_RESOLVE_REQUEST")
    if override:
        return Path(override).expanduser().resolve()
    if len(sys.argv) > 1 and sys.argv[1] and str(sys.argv[1]).lower().endswith(".json"):
        return Path(sys.argv[1]).expanduser().resolve()
    return DEFAULT_REQUEST_PATH


def resolve_output_path(request, request_path):
    explicit = request.get("resultPath")
    if explicit:
        return Path(explicit).expanduser().resolve()
    evidence_dir = request.get("evidenceDir")
    if evidence_dir:
        return Path(evidence_dir).expanduser().resolve() / "resolve-result.json"
    return request_path.parent / "resolve-result.json"


def load_resolve():
    in_process = globals().get("resolve")
    if in_process is not None and callable(getattr(in_process, "GetProjectManager", None)):
        return in_process

    bmd_module = globals().get("bmd")
    if bmd_module is not None and callable(getattr(bmd_module, "scriptapp", None)):
        in_process = bmd_module.scriptapp("Resolve")
        if in_process is not None:
            return in_process

    try:
        import DaVinciResolveScript as dvr_script
    except ImportError:
        modules_path = Path(
            "/Library/Application Support/Blackmagic Design/DaVinci Resolve/Developer/Scripting/Modules"
        )
        if str(modules_path) not in sys.path:
            sys.path.insert(0, str(modules_path))
        import DaVinciResolveScript as dvr_script

    application = dvr_script.scriptapp("Resolve")
    require(application is not None, "Resolve scripting application is unavailable")
    return application


def path_list(request):
    values = request.get("mediaPaths") or []
    if isinstance(values, str):
        values = [values]
    else:
        values = list(values)
    media_path = request.get("mediaPath")
    if media_path and Path(media_path).expanduser().is_file():
        values.append(media_path)
    return [str(Path(value).expanduser().resolve()) for value in values]


def source_clips_path(request):
    value = request.get("sourceClipsPath")
    if not value:
        media_path = request.get("mediaPath")
        if media_path and Path(media_path).expanduser().is_dir():
            value = media_path
    if not value:
        paths = path_list(request)
        parents = {str(Path(path).parent) for path in paths}
        if len(parents) == 1:
            value = next(iter(parents))
    return str(Path(value).expanduser().resolve()) if value else None


def folder_snapshot(folder):
    clips = []
    for clip in call_optional(folder, "GetClipList") or []:
        properties = call_optional(clip, "GetClipProperty") or {}
        clips.append(
            {
                "name": call_optional(clip, "GetName"),
                "uniqueId": call_optional(clip, "GetUniqueId"),
                "properties": properties,
            }
        )
    children = [folder_snapshot(child) for child in call_optional(folder, "GetSubFolderList") or []]
    return {
        "name": call_optional(folder, "GetName"),
        "uniqueId": call_optional(folder, "GetUniqueId"),
        "clips": clips,
        "folders": children,
    }


def linked_item_snapshot(item):
    return {
        "name": call_optional(item, "GetName"),
        "uniqueId": call_optional(item, "GetUniqueId"),
        "track": call_optional(item, "GetTrackTypeAndIndex"),
        "recordStart": call_optional(item, "GetStart", True),
        "recordEnd": call_optional(item, "GetEnd", True),
    }


def timeline_item_snapshot(item):
    media_pool_item = call_optional(item, "GetMediaPoolItem")
    media_properties = {}
    if media_pool_item and not isinstance(media_pool_item, dict):
        media_properties = call_optional(media_pool_item, "GetClipProperty") or {}
    linked_items = call_optional(item, "GetLinkedItems") or []
    if isinstance(linked_items, dict):
        linked_items = []
    return {
        "name": call_optional(item, "GetName"),
        "uniqueId": call_optional(item, "GetUniqueId"),
        "track": call_optional(item, "GetTrackTypeAndIndex"),
        "recordStart": call_optional(item, "GetStart", True),
        "recordEnd": call_optional(item, "GetEnd", True),
        "duration": call_optional(item, "GetDuration", True),
        "sourceStartFrame": call_optional(item, "GetSourceStartFrame"),
        "sourceEndFrame": call_optional(item, "GetSourceEndFrame"),
        "sourceStartTime": call_optional(item, "GetSourceStartTime"),
        "sourceEndTime": call_optional(item, "GetSourceEndTime"),
        "enabled": call_optional(item, "GetClipEnabled"),
        "properties": call_optional(item, "GetProperty") or {},
        "mediaPoolItem": {
            "name": call_optional(media_pool_item, "GetName") if media_pool_item else None,
            "uniqueId": call_optional(media_pool_item, "GetUniqueId") if media_pool_item else None,
            "properties": media_properties,
        },
        "linkedItems": [linked_item_snapshot(linked) for linked in linked_items],
        "sourceAudioChannelMapping": call_optional(item, "GetSourceAudioChannelMapping"),
    }


def timeline_snapshot(timeline):
    tracks = {}
    for track_type in ("video", "audio", "subtitle"):
        track_list = []
        count = int(call_optional(timeline, "GetTrackCount", track_type) or 0)
        for index in range(1, count + 1):
            items = call_optional(timeline, "GetItemListInTrack", track_type, index) or []
            track_list.append(
                {
                    "index": index,
                    "name": call_optional(timeline, "GetTrackName", track_type, index),
                    "subType": call_optional(timeline, "GetTrackSubType", track_type, index),
                    "enabled": call_optional(timeline, "GetIsTrackEnabled", track_type, index),
                    "locked": call_optional(timeline, "GetIsTrackLocked", track_type, index),
                    "items": [timeline_item_snapshot(item) for item in items],
                }
            )
        tracks[track_type] = track_list
    return {
        "name": call_optional(timeline, "GetName"),
        "uniqueId": call_optional(timeline, "GetUniqueId"),
        "startFrame": call_optional(timeline, "GetStartFrame"),
        "endFrame": call_optional(timeline, "GetEndFrame"),
        "startTimecode": call_optional(timeline, "GetStartTimecode"),
        "settings": call_optional(timeline, "GetSetting") or {},
        "tracks": tracks,
    }


def find_format_and_codec(project, request):
    formats = call_optional(project, "GetRenderFormats") or {}
    requested_format = request.get("renderFormat")
    if requested_format:
        render_format = requested_format
    else:
        render_format = next(
            (
                key
                for key, extension in formats.items()
                if str(key).lower() == "mp4" or str(extension).lower().lstrip(".") == "mp4"
            ),
            None,
        )
    require(render_format, "Resolve does not report an MP4 render format: %r" % formats)

    codecs = call_optional(project, "GetRenderCodecs", render_format) or {}
    requested_codec = request.get("renderCodec")
    if requested_codec:
        render_codec = requested_codec
    else:
        render_codec = next(
            (
                value
                for description, value in codecs.items()
                if "264" in str(description) or "264" in str(value)
            ),
            None,
        )
    require(render_codec, "Resolve does not report an H.264 codec for %s: %r" % (render_format, codecs))
    return render_format, render_codec, formats, codecs


def render_timeline(project, timeline, output_dir, request, report):
    render_format, render_codec, formats, codecs = find_format_and_codec(project, request)
    require(project.SetCurrentTimeline(timeline), "Could not select imported timeline")
    require(project.SetCurrentRenderMode(1), "Could not set single-clip render mode")
    require(
        project.SetCurrentRenderFormatAndCodec(render_format, render_codec),
        "Could not select render format %s / codec %s" % (render_format, render_codec),
    )
    custom_name = request.get("renderName") or "resolve-roundtrip"
    settings = {
        "SelectAllFrames": True,
        "TargetDir": str(output_dir),
        "CustomName": custom_name,
        "ExportVideo": True,
        "ExportAudio": bool(request.get("exportAudio", True)),
        "ReplaceExistingFilesInPlace": True,
    }
    settings.update(request.get("renderSettings") or {})
    require(project.SetRenderSettings(settings), "Resolve rejected render settings: %r" % settings)

    before = {str(path) for path in output_dir.glob("*") if path.is_file()}
    job_id = project.AddRenderJob()
    require(job_id, "Resolve did not create a render job")
    require(project.StartRendering(job_id), "Resolve did not start render job %s" % job_id)
    timeout = float(request.get("renderTimeoutSeconds", 600))
    started_at = time.monotonic()
    statuses = []
    while project.IsRenderingInProgress():
        status = call_optional(project, "GetRenderJobStatus", job_id) or {}
        statuses.append(json_safe(status))
        if time.monotonic() - started_at > timeout:
            project.StopRendering()
            raise TimeoutError("Resolve render exceeded %.1f seconds" % timeout)
        time.sleep(0.5)
    final_status = call_optional(project, "GetRenderJobStatus", job_id) or {}
    job_list = call_optional(project, "GetRenderJobList") or []
    after = {str(path) for path in output_dir.glob("*") if path.is_file()}
    created = sorted(after - before)
    if not created:
        created = sorted(
            str(path)
            for path in output_dir.glob(custom_name + "*")
            if path.is_file()
        )
    report.update(
        {
            "jobId": job_id,
            "format": render_format,
            "codec": render_codec,
            "availableFormats": formats,
            "availableCodecs": codecs,
            "settings": settings,
            "statusSamples": statuses[-20:],
            "finalStatus": final_status,
            "jobList": job_list,
            "artifacts": [
                {"path": path, "bytes": Path(path).stat().st_size} for path in created
            ],
        }
    )
    status_text = str((final_status or {}).get("JobStatus", "")).lower()
    require("complete" in status_text, "Resolve render did not complete: %r" % final_status)
    require(created, "Resolve completed render but no output artifact was found")
    require(all(Path(path).stat().st_size > 0 for path in created), "Resolve rendered an empty artifact")


def export_fcpxml(resolve, timeline, output_path):
    export_type = None
    export_version = None
    for version in ("1_10", "1_9", "1_8"):
        candidate = getattr(resolve, "EXPORT_FCPXML_" + version, None)
        if candidate is not None:
            export_type = candidate
            export_version = version.replace("_", ".")
            break
    require(export_type is not None, "Resolve API exposes no FCPXML export constant")
    export_subtype = getattr(resolve, "EXPORT_NONE", 0)
    require(
        timeline.Export(str(output_path), export_type, export_subtype),
        "Resolve failed to export FCPXML %s" % output_path,
    )
    require(output_path.exists() and output_path.stat().st_size > 0, "Resolve returned success without FCPXML output")
    return {"path": str(output_path), "bytes": output_path.stat().st_size, "version": export_version}


def run():
    request_path = resolve_request_path()
    request = {}
    result_path = request_path.parent / "resolve-result.json"
    report = {
        "schemaVersion": 1,
        "status": "running",
        "startedAt": utc_now(),
        "requestPath": str(request_path),
        "steps": [],
        "cleanup": {"attempted": False, "projectDeleted": False, "previousProjectRestored": False},
    }
    resolve = None
    project_manager = None
    project = None
    project_name = None
    previous_project_name = None

    def step(name, details=None):
        report["steps"].append({"name": name, "at": utc_now(), "details": details or {}})
        write_json(result_path, report)

    try:
        require(request_path.exists(), "Request JSON does not exist: %s" % request_path)
        with request_path.open("r", encoding="utf-8") as handle:
            request = json.load(handle)
        require(isinstance(request, dict), "Request JSON root must be an object")
        result_path = resolve_output_path(request, request_path)
        report["request"] = request
        write_json(result_path, report)

        fcpxml_path = Path(request.get("fcpxmlPath", "")).expanduser().resolve()
        require(request.get("fcpxmlPath"), "request.fcpxmlPath is required")
        require(fcpxml_path.is_file(), "FCPXML does not exist: %s" % fcpxml_path)
        output_dir = Path(request.get("renderOutputDir") or result_path.parent / "render").expanduser().resolve()
        output_dir.mkdir(parents=True, exist_ok=True)
        roundtrip_path = Path(
            request.get("roundtripFcpxmlPath") or result_path.parent / "resolve-roundtrip.fcpxml"
        ).expanduser().resolve()
        roundtrip_path.parent.mkdir(parents=True, exist_ok=True)
        step("request-validated", {"fcpxmlPath": str(fcpxml_path), "renderOutputDir": str(output_dir)})

        resolve = load_resolve()
        report["resolve"] = {
            "productName": call_optional(resolve, "GetProductName"),
            "version": call_optional(resolve, "GetVersion"),
            "versionString": call_optional(resolve, "GetVersionString"),
            "currentPage": call_optional(resolve, "GetCurrentPage"),
        }
        project_manager = resolve.GetProjectManager()
        require(project_manager is not None, "Resolve project manager is unavailable")
        previous_project = project_manager.GetCurrentProject()
        previous_project_name = previous_project.GetName() if previous_project else None
        step("resolve-connected", report["resolve"])

        run_id = re.sub(r"[^A-Za-z0-9_-]", "_", str(request.get("runId") or uuid.uuid4().hex))[:64]
        candidate_project_name = PROJECT_PREFIX + run_id
        require(
            candidate_project_name not in (project_manager.GetProjectListInCurrentFolder() or []),
            "Disposable project name already exists",
        )
        project = project_manager.CreateProject(candidate_project_name)
        require(
            project is not None,
            "Resolve failed to create disposable project %s" % candidate_project_name,
        )
        require(project.GetName() == candidate_project_name, "Resolve created an unexpected project")
        # Assign only after successful creation so cleanup can never delete a
        # pre-existing project that happened to have a colliding run id.
        project_name = candidate_project_name
        report["project"] = {"name": project_name, "uniqueId": call_optional(project, "GetUniqueId")}
        step("disposable-project-created", report["project"])

        media_pool = project.GetMediaPool()
        require(media_pool is not None, "Disposable project has no media pool")
        root_folder = media_pool.GetRootFolder()
        explicit_media = path_list(request)
        missing_media = [path for path in explicit_media if not Path(path).is_file()]
        require(not missing_media, "Configured media files do not exist: %r" % missing_media)
        imported_media = []
        if explicit_media:
            imported_media = resolve.GetMediaStorage().AddItemListToMediaPool(explicit_media) or []
            require(imported_media, "Resolve failed to pre-import configured media")
        step("media-prepared", {"paths": explicit_media, "importedCount": len(imported_media)})

        timeline_name = request.get("timelineName") or ("Video Creater Round Trip " + run_id[:12])
        import_options = {"timelineName": timeline_name}
        use_folders = bool(request.get("useSourceClipsFolders", explicit_media and True))
        fallback_path = source_clips_path(request)
        if use_folders and explicit_media:
            import_options.update({"importSourceClips": False, "sourceClipsFolders": [root_folder]})
        else:
            import_options["importSourceClips"] = True
            if fallback_path:
                import_options["sourceClipsPath"] = fallback_path
        timeline = media_pool.ImportTimelineFromFile(str(fcpxml_path), import_options)
        require(timeline is not None, "Resolve rejected FCPXML import with options %r" % import_options)
        require(project.SetCurrentTimeline(timeline), "Resolve failed to select imported timeline")
        report["import"] = {"path": str(fcpxml_path), "options": import_options}
        report["timeline"] = timeline_snapshot(timeline)
        report["mediaPool"] = folder_snapshot(root_folder)
        step("timeline-imported", {"name": timeline.GetName(), "trackCounts": {
            kind: len(report["timeline"]["tracks"][kind]) for kind in ("video", "audio", "subtitle")
        }})

        report["roundtripExport"] = export_fcpxml(resolve, timeline, roundtrip_path)
        step("fcpxml-exported", report["roundtripExport"])

        report["render"] = {}
        render_timeline(project, timeline, output_dir, request, report["render"])
        step("timeline-rendered", {"artifacts": report["render"]["artifacts"]})

        report["status"] = "passed"
        report["finishedAt"] = utc_now()
    except Exception as error:
        report["status"] = "failed"
        report["finishedAt"] = utc_now()
        report["error"] = {
            "type": type(error).__name__,
            "message": str(error),
            "traceback": traceback.format_exc(),
        }
    finally:
        report["cleanup"]["attempted"] = project_name is not None
        if project_manager is not None and project_name is not None:
            try:
                current = project_manager.GetCurrentProject()
                if current is not None and current.GetName() == project_name:
                    report["cleanup"]["projectClosed"] = bool(project_manager.CloseProject(current))
                remaining = project_manager.GetProjectListInCurrentFolder() or []
                if project_name in remaining:
                    report["cleanup"]["projectDeleted"] = bool(project_manager.DeleteProject(project_name))
                else:
                    report["cleanup"]["projectDeleted"] = True
                if previous_project_name and previous_project_name != project_name:
                    restored = project_manager.LoadProject(previous_project_name)
                    report["cleanup"]["previousProjectRestored"] = restored is not None
                elif not previous_project_name:
                    report["cleanup"]["previousProjectRestored"] = True
            except Exception as cleanup_error:
                report["cleanup"]["error"] = "%s: %s" % (
                    type(cleanup_error).__name__,
                    cleanup_error,
                )
        report["cleanup"]["finishedAt"] = utc_now()
        if report["cleanup"]["attempted"] and not report["cleanup"]["projectDeleted"]:
            report["status"] = "failed"
            report.setdefault(
                "error",
                {
                    "type": "CleanupError",
                    "message": "Resolve disposable project could not be deleted",
                    "traceback": None,
                },
            )
        write_json(result_path, report)
        print("Video Creater Resolve round-trip: %s" % report["status"])
        print("Evidence: %s" % result_path)
    return 0 if report["status"] == "passed" else 1


if __name__ == "__main__":
    raise SystemExit(run())
