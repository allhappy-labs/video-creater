-- Video Creater real DaVinci Resolve FCPXML round-trip proof.
-- Runs in-process from Workspace > Scripts; no external scripting is required.

local ROOT = "/Users/olhapi/Movies/VideoCreaterNLEEvidence"
local FCPXML = ROOT .. "/nle-roundtrip-proof-davinci.fcpxml"
local MEDIA = {
    ROOT .. "/media/nle-source-av.mp4",
    ROOT .. "/media/nle-source-voiceover.wav",
}
local RENDER_DIR = ROOT .. "/render"
local RESULT = ROOT .. "/resolve-result.json"
local PROJECT_PREFIX = "VideoCreater_NLE_RoundTrip_"

-- Written before touching any Resolve API so a missing marker distinguishes a
-- script-host/parse failure from a workflow failure.
local boot_marker = io.open(ROOT .. "/lua-boot.txt", "wb")
if boot_marker then
    boot_marker:write(os.date("!%Y-%m-%dT%H:%M:%SZ"), "\n")
    boot_marker:close()
end

local function now()
    return os.date("!%Y-%m-%dT%H:%M:%SZ")
end

local function shell_quote(value)
    return "'" .. string.gsub(value, "'", "'\\''") .. "'"
end

local function file_size(path)
    local handle = io.open(path, "rb")
    if not handle then return nil end
    local size = handle:seek("end")
    handle:close()
    return size
end

local function json_escape(value)
    local replacements = {
        ['"'] = '\\"', ['\\'] = '\\\\', ['\b'] = '\\b', ['\f'] = '\\f',
        ['\n'] = '\\n', ['\r'] = '\\r', ['\t'] = '\\t',
    }
    return string.gsub(value, '[%z\1-\31\\"\b\f\n\r\t]', function(character)
        return replacements[character] or string.format("\\u%04x", string.byte(character))
    end)
end

local function is_array(value)
    local count, maximum = 0, 0
    for key, _ in pairs(value) do
        if type(key) ~= "number" or key < 1 or key ~= math.floor(key) then return false end
        count = count + 1
        if key > maximum then maximum = key end
    end
    return count > 0 and maximum == count
end

local function json_encode(value, seen)
    local kind = type(value)
    if kind == "nil" then return "null" end
    if kind == "boolean" then return value and "true" or "false" end
    if kind == "number" then
        if value ~= value or value == math.huge or value == -math.huge then return "null" end
        return tostring(value)
    end
    if kind == "string" then return '"' .. json_escape(value) .. '"' end
    if kind ~= "table" then return '"' .. json_escape(tostring(value)) .. '"' end
    seen = seen or {}
    if seen[value] then return '"<cycle>"' end
    seen[value] = true
    local output = {}
    if is_array(value) then
        for index = 1, #value do output[#output + 1] = json_encode(value[index], seen) end
        seen[value] = nil
        return "[" .. table.concat(output, ",") .. "]"
    end
    for key, item in pairs(value) do
        output[#output + 1] = json_encode(tostring(key), seen) .. ":" .. json_encode(item, seen)
    end
    table.sort(output)
    seen[value] = nil
    return "{" .. table.concat(output, ",") .. "}"
end

local function write_report(report)
    local temporary = RESULT .. ".tmp"
    local handle, error_message = io.open(temporary, "wb")
    if not handle then error("Could not write result: " .. tostring(error_message)) end
    handle:write(json_encode(report), "\n")
    handle:close()
    assert(os.rename(temporary, RESULT))
end

local function require_value(value, message)
    if not value then error(message, 2) end
    return value
end

local function safe_call(target, method_name, ...)
    if not target then return nil end
    local method = target[method_name]
    if type(method) ~= "function" then return nil end
    local result = {pcall(method, target, ...)}
    if not result[1] then return {error = tostring(result[2])} end
    table.remove(result, 1)
    if #result == 1 then return result[1] end
    return result
end

local function linked_snapshot(item)
    return {
        name = safe_call(item, "GetName"),
        uniqueId = safe_call(item, "GetUniqueId"),
        track = safe_call(item, "GetTrackTypeAndIndex"),
        recordStart = safe_call(item, "GetStart", true),
        recordEnd = safe_call(item, "GetEnd", true),
    }
end

local function item_snapshot(item)
    local media_item = safe_call(item, "GetMediaPoolItem")
    local linked = safe_call(item, "GetLinkedItems") or {}
    local linked_output = {}
    if type(linked) == "table" and not linked.error then
        for _, linked_item in ipairs(linked) do linked_output[#linked_output + 1] = linked_snapshot(linked_item) end
    end
    return {
        name = safe_call(item, "GetName"),
        uniqueId = safe_call(item, "GetUniqueId"),
        track = safe_call(item, "GetTrackTypeAndIndex"),
        recordStart = safe_call(item, "GetStart", true),
        recordEnd = safe_call(item, "GetEnd", true),
        duration = safe_call(item, "GetDuration", true),
        sourceStartFrame = safe_call(item, "GetSourceStartFrame"),
        sourceEndFrame = safe_call(item, "GetSourceEndFrame"),
        sourceStartTime = safe_call(item, "GetSourceStartTime"),
        sourceEndTime = safe_call(item, "GetSourceEndTime"),
        enabled = safe_call(item, "GetClipEnabled"),
        properties = safe_call(item, "GetProperty") or {},
        linkedItems = linked_output,
        sourceAudioChannelMapping = safe_call(item, "GetSourceAudioChannelMapping"),
        mediaPoolItem = media_item and {
            name = safe_call(media_item, "GetName"),
            uniqueId = safe_call(media_item, "GetUniqueId"),
            properties = safe_call(media_item, "GetClipProperty") or {},
        } or {},
    }
end

local function timeline_snapshot(timeline)
    local tracks = {video = {}, audio = {}, subtitle = {}}
    for _, track_type in ipairs({"video", "audio", "subtitle"}) do
        local count = tonumber(safe_call(timeline, "GetTrackCount", track_type)) or 0
        for index = 1, count do
            local item_output = {}
            local items = safe_call(timeline, "GetItemListInTrack", track_type, index) or {}
            if type(items) == "table" and not items.error then
                for _, item in ipairs(items) do item_output[#item_output + 1] = item_snapshot(item) end
            end
            tracks[track_type][#tracks[track_type] + 1] = {
                index = index,
                name = safe_call(timeline, "GetTrackName", track_type, index),
                subType = safe_call(timeline, "GetTrackSubType", track_type, index),
                enabled = safe_call(timeline, "GetIsTrackEnabled", track_type, index),
                locked = safe_call(timeline, "GetIsTrackLocked", track_type, index),
                items = item_output,
            }
        end
    end
    return {
        name = safe_call(timeline, "GetName"),
        uniqueId = safe_call(timeline, "GetUniqueId"),
        startFrame = safe_call(timeline, "GetStartFrame"),
        endFrame = safe_call(timeline, "GetEndFrame"),
        startTimecode = safe_call(timeline, "GetStartTimecode"),
        settings = safe_call(timeline, "GetSetting") or {},
        tracks = tracks,
    }
end

local function folder_snapshot(folder)
    local clips, folders = {}, {}
    local clip_list = safe_call(folder, "GetClipList") or {}
    if type(clip_list) == "table" and not clip_list.error then
        for _, clip in ipairs(clip_list) do
            clips[#clips + 1] = {
                name = safe_call(clip, "GetName"),
                uniqueId = safe_call(clip, "GetUniqueId"),
                properties = safe_call(clip, "GetClipProperty") or {},
            }
        end
    end
    local child_list = safe_call(folder, "GetSubFolderList") or {}
    if type(child_list) == "table" and not child_list.error then
        for _, child in ipairs(child_list) do folders[#folders + 1] = folder_snapshot(child) end
    end
    return {name = safe_call(folder, "GetName"), clips = clips, folders = folders}
end

local function available_render_choice(project)
    local formats = require_value(project:GetRenderFormats(), "Resolve reported no render formats")
    local selected_format = nil
    for key, extension in pairs(formats) do
        if string.lower(tostring(key)) == "mp4" or string.lower(tostring(extension)):gsub("^%.", "") == "mp4" then
            selected_format = key
            break
        end
    end
    require_value(selected_format, "Resolve reported no MP4 render format")
    local codecs = require_value(project:GetRenderCodecs(selected_format), "Resolve reported no MP4 codecs")
    local selected_codec = nil
    for description, value in pairs(codecs) do
        if string.find(tostring(description), "264") or string.find(tostring(value), "264") then
            selected_codec = value
            break
        end
    end
    require_value(selected_codec, "Resolve reported no H.264 codec")
    return selected_format, selected_codec, formats, codecs
end

local function scan_artifacts(directory, prefix)
    local artifacts = {}
    local command = "/usr/bin/find " .. shell_quote(directory) .. " -maxdepth 1 -type f -name " .. shell_quote(prefix .. "*") .. " -print"
    local pipe = io.popen(command, "r")
    if pipe then
        for path in pipe:lines() do artifacts[#artifacts + 1] = {path = path, bytes = file_size(path) or 0} end
        pipe:close()
    end
    return artifacts
end

local report = {
    schemaVersion = 1,
    status = "running",
    startedAt = now(),
    requestPath = ROOT .. "/request.json",
    steps = {},
    cleanup = {attempted = false, projectDeleted = false, previousProjectRestored = false},
}

local project_manager, created_project, created_project_name, previous_project_name

local function step(name, details)
    report.steps[#report.steps + 1] = {name = name, at = now(), details = details or {}}
    write_report(report)
end

local function workflow()
    require_value(resolve, "This script must run inside DaVinci Resolve")
    require_value(file_size(FCPXML), "FCPXML fixture is missing: " .. FCPXML)
    for _, media_path in ipairs(MEDIA) do require_value(file_size(media_path), "Media fixture is missing: " .. media_path) end
    os.execute("/bin/mkdir -p " .. shell_quote(RENDER_DIR))
    step("request-validated", {fcpxmlPath = FCPXML, mediaPaths = MEDIA, renderOutputDir = RENDER_DIR})

    project_manager = require_value(resolve:GetProjectManager(), "Resolve project manager is unavailable")
    local previous_project = project_manager:GetCurrentProject()
    previous_project_name = previous_project and previous_project:GetName() or nil
    report.resolve = {
        productName = safe_call(resolve, "GetProductName"),
        version = safe_call(resolve, "GetVersion"),
        versionString = safe_call(resolve, "GetVersionString"),
    }
    step("resolve-connected", report.resolve)

    math.randomseed(os.time())
    local candidate_name = PROJECT_PREFIX .. tostring(os.time()) .. "_" .. tostring(math.random(100000, 999999))
    for _, existing in ipairs(project_manager:GetProjectListInCurrentFolder() or {}) do
        if existing == candidate_name then error("Generated disposable project name already exists") end
    end
    created_project = require_value(project_manager:CreateProject(candidate_name), "Could not create disposable project")
    require_value(created_project:GetName() == candidate_name, "Resolve created an unexpected project")
    created_project_name = candidate_name
    report.project = {name = candidate_name, uniqueId = safe_call(created_project, "GetUniqueId")}
    step("disposable-project-created", report.project)

    local media_pool = require_value(created_project:GetMediaPool(), "Disposable project has no media pool")
    local root_folder = require_value(media_pool:GetRootFolder(), "Disposable project has no root Media Pool folder")
    local imported_media = resolve:GetMediaStorage():AddItemListToMediaPool(MEDIA)
    require_value(imported_media and #imported_media > 0, "Resolve failed to pre-import fixture media")
    step("media-prepared", {paths = MEDIA, importedCount = #imported_media})

    -- Retain one Resolve-authored FCPXML sample beside failures so importer
    -- compatibility defects can be compared against the exact installed NLE.
    local control_timeline = require_value(
        media_pool:CreateTimelineFromClips("Video Creater Resolve Control", {imported_media[1]}),
        "Resolve failed to create the control timeline"
    )
    local control_token = tostring(os.time()) .. "_" .. tostring(math.random(100000, 999999))
    local control_path = ROOT .. "/resolve-control-" .. control_token .. ".fcpxml"
    local control_export_type = resolve.EXPORT_FCPXML_1_10 or resolve.EXPORT_FCPXML_1_9 or resolve.EXPORT_FCPXML_1_8
    require_value(control_timeline:Export(control_path, control_export_type, resolve.EXPORT_NONE), "Resolve failed to export control FCPXML")
    require_value(media_pool:DeleteTimelines({control_timeline}), "Resolve failed to delete the control timeline")
    report.resolveControlExport = {
        path = control_path .. "/Info.fcpxml",
        bytes = file_size(control_path .. "/Info.fcpxml"),
    }
    step("resolve-control-exported", report.resolveControlExport)

    local import_options = {
        timelineName = "Video Creater NLE Roundtrip Proof",
        importSourceClips = false,
        sourceClipsFolders = {root_folder},
    }
    local timeline = require_value(
        media_pool:ImportTimelineFromFile(FCPXML, import_options),
        "Resolve rejected the Video Creater FCPXML"
    )
    require_value(created_project:SetCurrentTimeline(timeline), "Could not select imported timeline")
    report.import = {path = FCPXML, options = {timelineName = import_options.timelineName, importSourceClips = false, sourceClipsFolders = {"Master"}}}
    report.timeline = timeline_snapshot(timeline)
    report.mediaPool = folder_snapshot(root_folder)
    step("timeline-imported", {name = timeline:GetName()})

    local video_items = report.timeline.tracks.video[1] and report.timeline.tracks.video[1].items or {}
    require_value(report.timeline.endFrame == 120, "Imported timeline must end at frame 120")
    require_value(#video_items == 2, "Imported timeline must contain exactly two video cuts")
    local expected_video = {
        {recordStart = 0, recordEnd = 48, sourceStartFrame = 24, sourceEndFrame = 72},
        {recordStart = 48, recordEnd = 120, sourceStartFrame = 96, sourceEndFrame = 168},
    }
    for index, expected in ipairs(expected_video) do
        local actual = video_items[index]
        for key, value in pairs(expected) do
            require_value(actual[key] == value, "Video cut " .. index .. " has incorrect " .. key)
        end
    end
    local voiceover = nil
    for _, track in ipairs(report.timeline.tracks.audio) do
        for _, item in ipairs(track.items) do
            if item.name == "nle-source-voiceover.wav" then voiceover = item end
        end
    end
    require_value(voiceover, "Imported timeline is missing the independent voiceover")
    require_value(
        voiceover.recordStart == 12 and voiceover.recordEnd == 108
            and voiceover.sourceStartFrame == 36 and voiceover.sourceEndFrame == 132,
        "Imported voiceover timing does not match the Video Creater edit"
    )
    report.semanticVerification = {
        fps = 24, durationFrames = 120, videoCutCount = 2,
        voiceoverRecordRange = {12, 108}, voiceoverSourceRange = {36, 132},
    }
    step("timeline-semantics-verified", report.semanticVerification)

    local artifact_token = tostring(os.time()) .. "_" .. tostring(math.random(100000, 999999))
    local roundtrip_container = ROOT .. "/resolve-roundtrip-" .. artifact_token .. ".fcpxml"
    local export_type = resolve.EXPORT_FCPXML_1_10 or resolve.EXPORT_FCPXML_1_9 or resolve.EXPORT_FCPXML_1_8
    require_value(export_type, "Resolve exposes no FCPXML export constant")
    require_value(timeline:Export(roundtrip_container, export_type, resolve.EXPORT_NONE), "Resolve failed to export round-trip FCPXML")
    local roundtrip_path = roundtrip_container .. "/Info.fcpxml"
    require_value(file_size(roundtrip_path), "Resolve returned success without a round-trip FCPXML artifact")
    report.roundtripExport = {path = roundtrip_path, bytes = file_size(roundtrip_path)}
    step("fcpxml-exported", report.roundtripExport)

    local render_format, render_codec, formats, codecs = available_render_choice(created_project)
    local render_name = "resolve-roundtrip-" .. artifact_token
    require_value(created_project:SetCurrentTimeline(timeline), "Could not select timeline for render")
    require_value(created_project:SetCurrentRenderMode(1), "Could not set single-clip render mode")
    require_value(created_project:SetCurrentRenderFormatAndCodec(render_format, render_codec), "Could not set MP4/H.264")
    local render_settings = {
        TargetDir = RENDER_DIR, CustomName = render_name,
    }
    require_value(created_project:SetRenderSettings(render_settings), "Resolve rejected render settings")
    local job_id = require_value(created_project:AddRenderJob(), "Resolve did not add a render job")
    require_value(created_project:StartRendering(job_id), "Resolve did not start the render")
    local render_started = os.time()
    local statuses = {}
    while created_project:IsRenderingInProgress() do
        statuses[#statuses + 1] = safe_call(created_project, "GetRenderJobStatus", job_id) or {}
        if os.difftime(os.time(), render_started) > 180 then
            created_project:StopRendering()
            error("Resolve render exceeded 180 seconds")
        end
        os.execute("/bin/sleep 1")
    end
    local final_status = safe_call(created_project, "GetRenderJobStatus", job_id) or {}
    local artifacts = scan_artifacts(RENDER_DIR, render_name)
    require_value(#artifacts > 0 and artifacts[1].bytes > 0, "Resolve produced no nonempty render artifact")
    require_value(string.find(string.lower(tostring(final_status.JobStatus or "")), "complete"), "Resolve render did not complete")
    report.render = {
        jobId = job_id, format = render_format, codec = render_codec,
        availableFormats = formats, availableCodecs = codecs, settings = render_settings,
        statusSamples = statuses, finalStatus = final_status, artifacts = artifacts,
    }
    step("timeline-rendered", {artifacts = artifacts})
end

local ok, error_message = xpcall(workflow, function(message)
    return {message = tostring(message), traceback = debug.traceback(tostring(message), 2)}
end)

if ok then
    report.status = "passed"
else
    report.status = "failed"
    report.error = {type = "LuaError", message = error_message.message, traceback = error_message.traceback}
end
report.finishedAt = now()

report.cleanup.attempted = created_project_name ~= nil
if created_project_name and project_manager then
    local cleanup_ok, cleanup_error = pcall(function()
        local current = project_manager:GetCurrentProject()
        if current and current:GetName() == created_project_name then
            report.cleanup.projectClosed = project_manager:CloseProject(current) and true or false
        end
        local present = false
        for _, name in ipairs(project_manager:GetProjectListInCurrentFolder() or {}) do
            if name == created_project_name then present = true end
        end
        report.cleanup.projectDeleted = not present or (project_manager:DeleteProject(created_project_name) and true or false)
        if previous_project_name and previous_project_name ~= created_project_name then
            report.cleanup.previousProjectRestored = project_manager:LoadProject(previous_project_name) ~= nil
        elseif not previous_project_name then
            report.cleanup.previousProjectRestored = true
        end
    end)
    if not cleanup_ok then report.cleanup.error = tostring(cleanup_error) end
end
report.cleanup.finishedAt = now()
if report.cleanup.attempted and not report.cleanup.projectDeleted then
    report.status = "failed"
    report.error = report.error or {type = "CleanupError", message = "Disposable project could not be deleted"}
end
write_report(report)

print("Video Creater Resolve round-trip: " .. report.status)
print("Evidence: " .. RESULT)
if report.status ~= "passed" then error(report.error.message) end
