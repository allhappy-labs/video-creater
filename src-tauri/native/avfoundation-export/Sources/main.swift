import AVFoundation
import CoreGraphics
import Foundation
import ImageIO
import QuartzCore

private let protocolName = "video-creater.avfoundation-export"
private let protocolVersion = 5
private let timescale: CMTimeScale = 60_000

private struct SourceProbeRequest: Decodable {
    let `protocol`: String
    let schemaVersion: Int
    let requestId: String
    let sourcePath: String
}

private struct SourceProbeResult: Encodable {
    let sourcePath: String
    let mediaType: String
    let durationSeconds: Double?
    let width: Int?
    let height: Int?
    let fps: Double?
}

private struct SourceProbeEvent: Encodable {
    let event: String
    let `protocol`: String
    let schemaVersion: Int
    let requestId: String?
    let result: SourceProbeResult?
    let error: WorkerError?
}

private struct ExportRequest: Decodable {
    let `protocol`: String
    let schemaVersion: Int
    let requestId: String
    let outputPath: String
    let profile: ExportProfile
    let quality: RenderQuality
    let width: Int
    let height: Int
    let fps: Double
    let videoClips: [VideoClip]
    let audioClips: [AudioClip]
    let overlays: [Overlay]
}

enum ExportProfile: String, Decodable {
    case h264
    case hevc
    case proResProxy
    case proRes422

    var presetName: String? {
        switch self {
        case .h264: AVAssetExportPresetHighestQuality
        case .hevc: AVAssetExportPresetHEVCHighestQuality
        case .proRes422: AVAssetExportPresetAppleProRes422LPCM
        case .proResProxy: nil
        }
    }

    var fileType: AVFileType {
        switch self {
        case .h264, .hevc: .mp4
        case .proResProxy, .proRes422: .mov
        }
    }
}

enum RenderQuality: String, Decodable {
    case draft
    case final
}

private func profile(for codec: ExportProfile, quality: RenderQuality) -> ExportProfile {
    if codec == .proRes422 && quality == .draft { return .proResProxy }
    return codec
}

func exportPresetName(
    profile codec: ExportProfile,
    quality: RenderQuality,
    width: Int,
    height: Int
) -> String? {
    let selectedProfile = profile(for: codec, quality: quality)
    guard quality == .draft else { return selectedProfile.presetName }
    switch selectedProfile {
    case .h264:
        if width <= 1280 && height <= 720 { return AVAssetExportPreset1280x720 }
        if width <= 1920 && height <= 1080 { return AVAssetExportPreset1920x1080 }
        if width <= 3840 && height <= 2160 { return AVAssetExportPreset3840x2160 }
        return nil
    case .hevc:
        if width <= 1920 && height <= 1080 { return AVAssetExportPresetHEVC1920x1080 }
        if width <= 3840 && height <= 2160 { return AVAssetExportPresetHEVC3840x2160 }
        return nil
    case .proResProxy:
        return nil
    case .proRes422:
        return selectedProfile.presetName
    }
}

private struct VideoClip: Decodable {
    let sourcePath: String
    let sourceStartSeconds: Double
    let sourceEndSeconds: Double
    let timelineStartSeconds: Double
    let timelineDurationSeconds: Double
    let trackIndex: Int
    let opacity: Double
    let fadeInSeconds: Double
    let fadeOutSeconds: Double
    let transform: CanvasTransform
    let crop: CropInsets
    let preparedFrames: PreparedFrames?
}

private struct PreparedFrames: Decodable {
    let framePaths: [String]
    let frameDurationSeconds: Double
    let alpha: Bool
}

private struct AudioClip: Decodable {
    let sourcePath: String
    let sourceStartSeconds: Double
    let sourceEndSeconds: Double
    let timelineStartSeconds: Double
    let timelineDurationSeconds: Double
    let trackIndex: Int
    let volumeDb: Double
    let fadeInSeconds: Double
    let fadeOutSeconds: Double
    let volumeKeyframes: [AudioVolumeKeyframe]?
}

private struct AudioVolumeKeyframe: Decodable {
    let atSeconds: Double
    let valueDb: Double
    let easing: String
}

private struct CanvasTransform: Decodable {
    let centerX: Double
    let centerY: Double
    let width: Double
    let height: Double
    let flipHorizontal: Bool
    let flipVertical: Bool
}

private struct CropInsets: Decodable {
    let top: Double
    let right: Double
    let bottom: Double
    let left: Double
}

private struct Overlay: Decodable {
    let framePaths: [String]
    let timelineStartSeconds: Double
    let durationSeconds: Double
    let frameDurationSeconds: Double
}

private struct WorkerEvent: Encodable {
    let event: String
    let `protocol`: String
    let schemaVersion: Int
    let requestId: String?
    let phase: String?
    let completed: Int?
    let total: Int?
    let message: String?
    let result: ExportResult?
    let error: WorkerError?

    static func progress(
        requestId: String?,
        phase: String,
        completed: Int,
        total: Int,
        message: String
    ) -> Self {
        Self(
            event: "progress",
            protocol: protocolName,
            schemaVersion: protocolVersion,
            requestId: requestId,
            phase: phase,
            completed: completed,
            total: total,
            message: message,
            result: nil,
            error: nil
        )
    }

    static func completed(requestId: String, result: ExportResult) -> Self {
        Self(
            event: "completed",
            protocol: protocolName,
            schemaVersion: protocolVersion,
            requestId: requestId,
            phase: nil,
            completed: nil,
            total: nil,
            message: nil,
            result: result,
            error: nil
        )
    }

    static func failed(requestId: String?, error: WorkerError) -> Self {
        Self(
            event: "failed",
            protocol: protocolName,
            schemaVersion: protocolVersion,
            requestId: requestId,
            phase: nil,
            completed: nil,
            total: nil,
            message: nil,
            result: nil,
            error: error
        )
    }
}

private struct ExportResult: Encodable {
    let outputPath: String
    let durationSeconds: Double
    let sizeBytes: UInt64
    let videoCodec: String
    let audioCodec: String?
    let width: Int
    let height: Int
    let fps: Double?
    let videoClipCount: Int
    let audioClipCount: Int
    let overlayCount: Int
}

private struct WorkerError: Encodable {
    let code: String
    let message: String
    let field: String?
    let retryable: Bool
}

private struct CapabilitiesReport: Encodable {
    let `protocol`: String
    let schemaVersion: Int
    let backend: String
    let profiles: [String: Bool]
}

private struct PreparedVideoClip {
    let request: VideoClip
    let track: AVMutableCompositionTrack
    let sourceTrack: AVAssetTrack
    let naturalSize: CGSize
    let preferredTransform: CGAffineTransform
}

private enum ExporterError: LocalizedError {
    case invalid(String, String?)
    case unavailable(String)
    case io(String)
    case export(String)

    var errorDescription: String? {
        switch self {
        case .invalid(let message, _), .unavailable(let message), .io(let message), .export(let message):
            message
        }
    }

    var code: String {
        switch self {
        case .invalid: "invalidRequest"
        case .unavailable: "backendUnavailable"
        case .io: "ioFailure"
        case .export: "exportFailed"
        }
    }

    var field: String? {
        if case .invalid(_, let field) = self { field } else { nil }
    }
}

@main
private enum VideoCreaterAVFoundationExport {
    static func main() async {
        if CommandLine.arguments.dropFirst().contains("--capabilities") {
            emitCapabilities()
            return
        }
        if CommandLine.arguments.dropFirst().contains("--source-probe") {
            await runSourceProbe()
            return
        }
        let decoder = JSONDecoder()
        var requestId: String?
        do {
            let data = FileHandle.standardInput.readDataToEndOfFile()
            guard !data.isEmpty else {
                throw ExporterError.invalid("stdin must contain one export request", "request")
            }
            let request = try decoder.decode(ExportRequest.self, from: data)
            requestId = request.requestId
            try validate(request)
            emit(.progress(
                requestId: request.requestId,
                phase: "accepted",
                completed: 0,
                total: 1,
                message: "AVFoundation export request accepted"
            ))
            let result = try await render(request)
            emit(.completed(requestId: request.requestId, result: result))
        } catch {
            let exporterError = error as? ExporterError
                ?? ExporterError.invalid(error.localizedDescription, nil)
            emit(.failed(
                requestId: requestId,
                error: WorkerError(
                    code: exporterError.code,
                    message: exporterError.localizedDescription,
                    field: exporterError.field,
                    retryable: false
                )
            ))
            Foundation.exit(1)
        }
    }
}

private func runSourceProbe() async {
    let decoder = JSONDecoder()
    var requestId: String?
    do {
        let data = FileHandle.standardInput.readDataToEndOfFile()
        guard !data.isEmpty else {
            throw ExporterError.invalid("stdin must contain one source probe request", "request")
        }
        let request = try decoder.decode(SourceProbeRequest.self, from: data)
        requestId = request.requestId
        guard request.protocol == protocolName else {
            throw ExporterError.invalid("protocol must be \(protocolName)", "protocol")
        }
        guard request.schemaVersion == protocolVersion else {
            throw ExporterError.invalid("unsupported schema version", "schemaVersion")
        }
        guard !request.requestId.trimmingCharacters(in: .whitespacesAndNewlines).isEmpty else {
            throw ExporterError.invalid("requestId must not be blank", "requestId")
        }
        let sourceURL = URL(fileURLWithPath: request.sourcePath)
        guard sourceURL.path == request.sourcePath, sourceURL.path.hasPrefix("/") else {
            throw ExporterError.invalid("sourcePath must be an absolute normalized path", "sourcePath")
        }
        var isDirectory: ObjCBool = false
        guard FileManager.default.fileExists(atPath: sourceURL.path, isDirectory: &isDirectory),
              !isDirectory.boolValue else {
            throw ExporterError.invalid("sourcePath must identify a file", "sourcePath")
        }
        let result = try await probeSource(sourceURL)
        emitEncodable(SourceProbeEvent(
            event: "completed",
            protocol: protocolName,
            schemaVersion: protocolVersion,
            requestId: request.requestId,
            result: result,
            error: nil
        ))
    } catch {
        let exporterError = error as? ExporterError
            ?? ExporterError.invalid(error.localizedDescription, nil)
        emitEncodable(SourceProbeEvent(
            event: "failed",
            protocol: protocolName,
            schemaVersion: protocolVersion,
            requestId: requestId,
            result: nil,
            error: WorkerError(
                code: exporterError.code,
                message: exporterError.localizedDescription,
                field: exporterError.field,
                retryable: false
            )
        ))
        Foundation.exit(1)
    }
}

private func probeSource(_ sourceURL: URL) async throws -> SourceProbeResult {
    if let imageSource = CGImageSourceCreateWithURL(sourceURL as CFURL, nil),
       CGImageSourceGetCount(imageSource) > 0,
       let properties = CGImageSourceCopyPropertiesAtIndex(imageSource, 0, nil) as? [CFString: Any],
       let width = properties[kCGImagePropertyPixelWidth] as? NSNumber,
       let height = properties[kCGImagePropertyPixelHeight] as? NSNumber,
       width.intValue > 0,
       height.intValue > 0 {
        return SourceProbeResult(
            sourcePath: sourceURL.path,
            mediaType: "image",
            durationSeconds: nil,
            width: width.intValue,
            height: height.intValue,
            fps: nil
        )
    }

    let asset = AVURLAsset(url: sourceURL)
    let duration = try await asset.load(.duration)
    let durationSeconds = CMTimeGetSeconds(duration)
    guard durationSeconds.isFinite, durationSeconds > 0 else {
        throw ExporterError.invalid("source duration is unavailable or invalid", "sourcePath")
    }
    let videoTracks = try await asset.loadTracks(withMediaType: .video)
    let audioTracks = try await asset.loadTracks(withMediaType: .audio)
    if let videoTrack = videoTracks.first {
        let naturalSize = try await videoTrack.load(.naturalSize)
        let preferredTransform = try await videoTrack.load(.preferredTransform)
        let displayedRect = CGRect(origin: .zero, size: naturalSize).applying(preferredTransform)
        let width = Int(abs(displayedRect.width).rounded())
        let height = Int(abs(displayedRect.height).rounded())
        let nominalFrameRate = try await videoTrack.load(.nominalFrameRate)
        guard width > 0, height > 0 else {
            throw ExporterError.invalid("video dimensions are unavailable", "sourcePath")
        }
        return SourceProbeResult(
            sourcePath: sourceURL.path,
            mediaType: "video",
            durationSeconds: durationSeconds,
            width: width,
            height: height,
            fps: nominalFrameRate.isFinite && nominalFrameRate > 0 ? Double(nominalFrameRate) : nil
        )
    }
    if !audioTracks.isEmpty {
        return SourceProbeResult(
            sourcePath: sourceURL.path,
            mediaType: "audio",
            durationSeconds: durationSeconds,
            width: nil,
            height: nil,
            fps: nil
        )
    }
    throw ExporterError.invalid("source contains no decodable image, video, or audio", "sourcePath")
}

private func emitCapabilities() {
    let presets = Set(AVAssetExportSession.allExportPresets())
    let report = CapabilitiesReport(
        protocol: protocolName,
        schemaVersion: protocolVersion,
        backend: "avfoundation-native",
        profiles: [
            "h264": presets.contains(AVAssetExportPresetHighestQuality),
            "hevc": presets.contains(AVAssetExportPresetHEVCHighestQuality),
            "h264Draft": presets.contains(AVAssetExportPreset1280x720)
                && presets.contains(AVAssetExportPreset1920x1080),
            "h264Final": presets.contains(AVAssetExportPresetHighestQuality),
            "hevcDraft": presets.contains(AVAssetExportPresetHEVC1920x1080),
            "hevcFinal": presets.contains(AVAssetExportPresetHEVCHighestQuality),
            "proResProxy": false,
            "proRes422": presets.contains(AVAssetExportPresetAppleProRes422LPCM),
        ]
    )
    let encoder = JSONEncoder()
    guard let data = try? encoder.encode(report) else {
        Foundation.exit(1)
    }
    FileHandle.standardOutput.write(data)
    FileHandle.standardOutput.write(Data([0x0A]))
}

private func validate(_ request: ExportRequest) throws {
    guard request.protocol == protocolName else {
        throw ExporterError.invalid("protocol must be \(protocolName)", "protocol")
    }
    guard request.schemaVersion == protocolVersion else {
        throw ExporterError.invalid("unsupported schema version", "schemaVersion")
    }
    guard !request.requestId.trimmingCharacters(in: .whitespacesAndNewlines).isEmpty else {
        throw ExporterError.invalid("requestId must not be blank", "requestId")
    }
    guard request.width > 0, request.width <= 16_384 else {
        throw ExporterError.invalid("width must be between 1 and 16384", "width")
    }
    guard request.height > 0, request.height <= 16_384 else {
        throw ExporterError.invalid("height must be between 1 and 16384", "height")
    }
    guard request.fps.isFinite, request.fps > 0, request.fps <= 240 else {
        throw ExporterError.invalid("fps must be finite and between 0 and 240", "fps")
    }
    guard !request.videoClips.isEmpty else {
        throw ExporterError.invalid("at least one video clip is required", "videoClips")
    }
    guard !request.outputPath.trimmingCharacters(in: .whitespacesAndNewlines).isEmpty else {
        throw ExporterError.invalid("outputPath must not be blank", "outputPath")
    }
    for (index, clip) in request.videoClips.enumerated() {
        try validateClipRange(
            sourcePath: clip.sourcePath,
            sourceStart: clip.sourceStartSeconds,
            sourceEnd: clip.sourceEndSeconds,
            timelineStart: clip.timelineStartSeconds,
            timelineDuration: clip.timelineDurationSeconds,
            field: "videoClips[\(index)]"
        )
        guard clip.opacity.isFinite, (0...1).contains(clip.opacity) else {
            throw ExporterError.invalid("opacity must be between 0 and 1", "videoClips[\(index)].opacity")
        }
        guard clip.fadeInSeconds >= 0, clip.fadeOutSeconds >= 0,
              clip.fadeInSeconds + clip.fadeOutSeconds <= clip.timelineDurationSeconds else {
            throw ExporterError.invalid("clip fades must fit inside its timeline duration", "videoClips[\(index)]")
        }
        try validate(clip.transform, field: "videoClips[\(index)].transform")
        try validate(clip.crop, field: "videoClips[\(index)].crop")
        if let frames = clip.preparedFrames {
            guard !frames.framePaths.isEmpty,
                  frames.frameDurationSeconds.isFinite,
                  frames.frameDurationSeconds > 0,
                  frames.framePaths.allSatisfy({ $0.hasPrefix("/") && FileManager.default.fileExists(atPath: $0) }) else {
                throw ExporterError.invalid("prepared frame sequence is invalid", "videoClips[\(index)].preparedFrames")
            }
        }
    }
    for (index, clip) in request.audioClips.enumerated() {
        try validateClipRange(
            sourcePath: clip.sourcePath,
            sourceStart: clip.sourceStartSeconds,
            sourceEnd: clip.sourceEndSeconds,
            timelineStart: clip.timelineStartSeconds,
            timelineDuration: clip.timelineDurationSeconds,
            field: "audioClips[\(index)]"
        )
        guard clip.volumeDb.isFinite, (-60...24).contains(clip.volumeDb) else {
            throw ExporterError.invalid("volumeDb must be between -60 and 24", "audioClips[\(index)].volumeDb")
        }
        guard clip.fadeInSeconds.isFinite, clip.fadeOutSeconds.isFinite,
              clip.fadeInSeconds >= 0, clip.fadeOutSeconds >= 0,
              clip.fadeInSeconds + clip.fadeOutSeconds <= clip.timelineDurationSeconds else {
            throw ExporterError.invalid("audio fades must fit inside the clip duration", "audioClips[\(index)]")
        }
        var previousTime: Double?
        for (pointIndex, point) in (clip.volumeKeyframes ?? []).enumerated() {
            guard point.atSeconds.isFinite,
                  point.atSeconds >= 0,
                  point.atSeconds <= clip.timelineDurationSeconds,
                  previousTime.map({ point.atSeconds > $0 }) ?? true,
                  point.valueDb.isFinite,
                  (-60...24).contains(point.valueDb),
                  ["linear", "hold", "easeIn", "easeOut", "easeInOut"].contains(point.easing) else {
                throw ExporterError.invalid(
                    "audio volume keyframe is invalid or out of order",
                    "audioClips[\(index)].volumeKeyframes[\(pointIndex)]"
                )
            }
            previousTime = point.atSeconds
        }
    }
    let duration = requestDuration(request)
    for (index, overlay) in request.overlays.enumerated() {
        guard !overlay.framePaths.isEmpty else {
            throw ExporterError.invalid("overlay framePaths must not be empty", "overlays[\(index)].framePaths")
        }
        guard overlay.timelineStartSeconds.isFinite, overlay.timelineStartSeconds >= 0,
              overlay.durationSeconds.isFinite, overlay.durationSeconds > 0,
              overlay.timelineStartSeconds + overlay.durationSeconds <= duration + 0.001 else {
            throw ExporterError.invalid("overlay timing must fit inside the export", "overlays[\(index)]")
        }
        guard overlay.frameDurationSeconds.isFinite, overlay.frameDurationSeconds > 0 else {
            throw ExporterError.invalid("overlay frame duration must be positive", "overlays[\(index)].frameDurationSeconds")
        }
    }
}

private func validateClipRange(
    sourcePath: String,
    sourceStart: Double,
    sourceEnd: Double,
    timelineStart: Double,
    timelineDuration: Double,
    field: String
) throws {
    guard !sourcePath.trimmingCharacters(in: .whitespacesAndNewlines).isEmpty else {
        throw ExporterError.invalid("sourcePath must not be blank", "\(field).sourcePath")
    }
    guard sourceStart.isFinite, sourceStart >= 0,
          sourceEnd.isFinite, sourceEnd > sourceStart else {
        throw ExporterError.invalid("source range is invalid", field)
    }
    guard timelineStart.isFinite, timelineStart >= 0,
          timelineDuration.isFinite, timelineDuration > 0 else {
        throw ExporterError.invalid("timeline range is invalid", field)
    }
}

private func validate(_ transform: CanvasTransform, field: String) throws {
    guard transform.centerX.isFinite, (0...1).contains(transform.centerX),
          transform.centerY.isFinite, (0...1).contains(transform.centerY),
          transform.width.isFinite, transform.width > 0, transform.width <= 1,
          transform.height.isFinite, transform.height > 0, transform.height <= 1 else {
        throw ExporterError.invalid("transform must use normalized canvas coordinates", field)
    }
}

private func validate(_ crop: CropInsets, field: String) throws {
    let values = [crop.top, crop.right, crop.bottom, crop.left]
    guard values.allSatisfy({ $0.isFinite && $0 >= 0 && $0 < 1 }),
          crop.left + crop.right < 1,
          crop.top + crop.bottom < 1 else {
        throw ExporterError.invalid("crop must leave visible normalized source content", field)
    }
}

private func render(_ request: ExportRequest) async throws -> ExportResult {
    let composition = AVMutableComposition()
    let renderSize = CGSize(width: request.width, height: request.height)
    var preparedVideo = [PreparedVideoClip]()
    var audioParameters = [AVMutableAudioMixInputParameters]()
    var temporaryPreparedAssets = [URL]()
    defer { temporaryPreparedAssets.forEach { try? FileManager.default.removeItem(at: $0) } }

    for clip in request.videoClips {
        let sourceURL: URL
        let stillImageSource = clip.preparedFrames == nil && isSupportedStillImage(clip.sourcePath)
        if let frames = clip.preparedFrames {
            sourceURL = try await writePreparedFrameAsset(
                frames,
                width: request.width,
                height: request.height,
                fps: request.fps
            )
            temporaryPreparedAssets.append(sourceURL)
        } else if stillImageSource {
            let frameCount = max(1, Int(ceil(clip.timelineDurationSeconds * request.fps)))
            sourceURL = try await writePreparedFrameAsset(
                PreparedFrames(
                    framePaths: Array(repeating: clip.sourcePath, count: frameCount),
                    frameDurationSeconds: 1 / request.fps,
                    alpha: true
                ),
                width: request.width,
                height: request.height,
                fps: request.fps
            )
            temporaryPreparedAssets.append(sourceURL)
        } else {
            sourceURL = URL(fileURLWithPath: clip.sourcePath)
        }
        let asset = AVURLAsset(url: sourceURL)
        guard let sourceTrack = try await asset.loadTracks(withMediaType: .video).first else {
            throw ExporterError.unavailable("video source has no video track: \(clip.sourcePath)")
        }
        guard let targetTrack = composition.addMutableTrack(
            withMediaType: .video,
            preferredTrackID: kCMPersistentTrackID_Invalid
        ) else {
            throw ExporterError.export("AVFoundation could not allocate a video composition track")
        }
        let sourceRange = CMTimeRange(
            start: time(stillImageSource ? 0 : clip.sourceStartSeconds),
            duration: time(stillImageSource
                ? clip.timelineDurationSeconds
                : clip.sourceEndSeconds - clip.sourceStartSeconds)
        )
        let timelineStart = time(clip.timelineStartSeconds)
        do {
            try targetTrack.insertTimeRange(sourceRange, of: sourceTrack, at: timelineStart)
            let inserted = CMTimeRange(start: timelineStart, duration: sourceRange.duration)
            let targetDuration = time(clip.timelineDurationSeconds)
            if abs(CMTimeGetSeconds(sourceRange.duration - targetDuration)) > 0.000_001 {
                targetTrack.scaleTimeRange(inserted, toDuration: targetDuration)
            }
        } catch {
            throw ExporterError.export("video clip could not be inserted: \(error.localizedDescription)")
        }
        preparedVideo.append(PreparedVideoClip(
            request: clip,
            track: targetTrack,
            sourceTrack: sourceTrack,
            naturalSize: try await sourceTrack.load(.naturalSize),
            preferredTransform: try await sourceTrack.load(.preferredTransform)
        ))
    }

    for clip in request.audioClips {
        let asset = AVURLAsset(url: URL(fileURLWithPath: clip.sourcePath))
        guard let sourceTrack = try await asset.loadTracks(withMediaType: .audio).first else {
            throw ExporterError.unavailable("audio source has no audio track: \(clip.sourcePath)")
        }
        guard let targetTrack = composition.addMutableTrack(
            withMediaType: .audio,
            preferredTrackID: kCMPersistentTrackID_Invalid
        ) else {
            throw ExporterError.export("AVFoundation could not allocate an audio composition track")
        }
        let sourceRange = CMTimeRange(
            start: time(clip.sourceStartSeconds),
            duration: time(clip.sourceEndSeconds - clip.sourceStartSeconds)
        )
        let timelineStart = time(clip.timelineStartSeconds)
        do {
            try targetTrack.insertTimeRange(sourceRange, of: sourceTrack, at: timelineStart)
            let targetDuration = time(clip.timelineDurationSeconds)
            if abs(CMTimeGetSeconds(sourceRange.duration - targetDuration)) > 0.000_001 {
                targetTrack.scaleTimeRange(
                    CMTimeRange(start: timelineStart, duration: sourceRange.duration),
                    toDuration: targetDuration
                )
            }
        } catch {
            throw ExporterError.export("audio clip could not be inserted: \(error.localizedDescription)")
        }
        let parameters = AVMutableAudioMixInputParameters(track: targetTrack)
        applyAudioGain(parameters: parameters, clip: clip, fps: request.fps)
        audioParameters.append(parameters)
    }

    let durationSeconds = requestDuration(request)
    let videoComposition = AVMutableVideoComposition()
    videoComposition.renderSize = renderSize
    videoComposition.frameDuration = time(1 / request.fps)
    let instruction = AVMutableVideoCompositionInstruction()
    instruction.timeRange = CMTimeRange(start: .zero, duration: time(durationSeconds))
    instruction.layerInstructions = preparedVideo
        .enumerated()
        .sorted {
            if $0.element.request.trackIndex != $1.element.request.trackIndex {
                return $0.element.request.trackIndex > $1.element.request.trackIndex
            }
            return $0.offset > $1.offset
        }
        .map { _, prepared in
            makeLayerInstruction(prepared, renderSize: renderSize)
        }
    videoComposition.instructions = [instruction]

    if !request.overlays.isEmpty {
        videoComposition.animationTool = try makeAnimationTool(
            overlays: request.overlays,
            renderSize: renderSize,
            durationSeconds: durationSeconds
        )
    }

    let audioMix = AVMutableAudioMix()
    audioMix.inputParameters = audioParameters
    let outputURL = URL(fileURLWithPath: request.outputPath)
    try FileManager.default.createDirectory(
        at: outputURL.deletingLastPathComponent(),
        withIntermediateDirectories: true
    )
    if FileManager.default.fileExists(atPath: outputURL.path) {
        try FileManager.default.removeItem(at: outputURL)
    }
    let selectedProfile = profile(for: request.profile, quality: request.quality)
    guard let presetName = exportPresetName(
        profile: request.profile,
        quality: request.quality,
        width: request.width,
        height: request.height
    ) else {
        throw ExporterError.unavailable("AVFoundation has no approved \(request.quality.rawValue) preset for the selected codec and dimensions")
    }
    guard AVAssetExportSession.allExportPresets().contains(presetName) else {
        throw ExporterError.unavailable("AVFoundation export preset is unavailable: \(presetName)")
    }
    guard let exporter = AVAssetExportSession(asset: composition, presetName: presetName) else {
        throw ExporterError.unavailable("AVFoundation export preset is unavailable: \(presetName)")
    }
    guard exporter.supportedFileTypes.contains(selectedProfile.fileType) else {
        throw ExporterError.unavailable("AVFoundation export file type is unavailable for the selected profile")
    }
    exporter.videoComposition = videoComposition
    exporter.audioMix = audioParameters.isEmpty ? nil : audioMix
    exporter.shouldOptimizeForNetworkUse = selectedProfile != .proRes422 && selectedProfile != .proResProxy
    do {
        try await exporter.export(to: outputURL, as: selectedProfile.fileType)
    } catch {
        throw ExporterError.export("AVFoundation export failed: \(error.localizedDescription)")
    }
    guard FileManager.default.fileExists(atPath: outputURL.path) else {
        throw ExporterError.io("AVFoundation reported success without writing the output file")
    }
    let probe = try await probeOutput(outputURL)
    return ExportResult(
        outputPath: outputURL.path,
        durationSeconds: probe.durationSeconds,
        sizeBytes: probe.sizeBytes,
        videoCodec: probe.videoCodec,
        audioCodec: probe.audioCodec,
        width: probe.width,
        height: probe.height,
        fps: probe.fps,
        videoClipCount: request.videoClips.count,
        audioClipCount: request.audioClips.count,
        overlayCount: request.overlays.count
    )
}

private func applyAudioGain(
    parameters: AVMutableAudioMixInputParameters,
    clip: AudioClip,
    fps: Double
) {
    let keyframes = clip.volumeKeyframes ?? []
    let timelineStart = clip.timelineStartSeconds
    if keyframes.isEmpty {
        let targetVolume = Float(pow(10, clip.volumeDb / 20))
        if clip.fadeInSeconds > 0 {
            parameters.setVolumeRamp(
                fromStartVolume: 0,
                toEndVolume: targetVolume,
                timeRange: CMTimeRange(start: time(timelineStart), duration: time(clip.fadeInSeconds))
            )
        } else {
            parameters.setVolume(targetVolume, at: time(timelineStart))
        }
        if clip.fadeOutSeconds > 0 {
            let fadeOutStart = timelineStart + clip.timelineDurationSeconds - clip.fadeOutSeconds
            parameters.setVolumeRamp(
                fromStartVolume: targetVolume,
                toEndVolume: 0,
                timeRange: CMTimeRange(start: time(fadeOutStart), duration: time(clip.fadeOutSeconds))
            )
        }
        return
    }

    // AVAudioMix ramps are linear in amplitude, while the editor interpolates
    // automation in dB. Sample the canonical curve at frame cadence and emit
    // short amplitude ramps so preview and export share the same envelope.
    let sampleRate = min(60.0, max(30.0, fps))
    let segmentCount = max(
        1,
        min(20_000, Int(ceil(clip.timelineDurationSeconds * sampleRate)))
    )
    for segment in 0..<segmentCount {
        let localStart = clip.timelineDurationSeconds * Double(segment) / Double(segmentCount)
        let localEnd = clip.timelineDurationSeconds * Double(segment + 1) / Double(segmentCount)
        parameters.setVolumeRamp(
            fromStartVolume: audioGain(clip: clip, localSeconds: localStart),
            toEndVolume: audioGain(clip: clip, localSeconds: localEnd),
            timeRange: CMTimeRange(
                start: time(timelineStart + localStart),
                duration: time(localEnd - localStart)
            )
        )
    }
}

private func audioGain(clip: AudioClip, localSeconds: Double) -> Float {
    let volumeDb = sampledVolumeDb(clip: clip, localSeconds: localSeconds)
    let fadeIn = clip.fadeInSeconds > 0
        ? min(1, max(0, localSeconds / clip.fadeInSeconds))
        : 1
    let fadeOut = clip.fadeOutSeconds > 0
        ? min(1, max(0, (clip.timelineDurationSeconds - localSeconds) / clip.fadeOutSeconds))
        : 1
    return Float(pow(10, volumeDb / 20) * fadeIn * fadeOut)
}

private func sampledVolumeDb(clip: AudioClip, localSeconds: Double) -> Double {
    let keyframes = clip.volumeKeyframes ?? []
    guard let first = keyframes.first else { return clip.volumeDb }
    if localSeconds <= first.atSeconds { return first.valueDb }
    guard let last = keyframes.last else { return clip.volumeDb }
    if localSeconds >= last.atSeconds { return last.valueDb }
    for index in 0..<(keyframes.count - 1) {
        let current = keyframes[index]
        let next = keyframes[index + 1]
        guard localSeconds >= current.atSeconds, localSeconds <= next.atSeconds else { continue }
        let span = next.atSeconds - current.atSeconds
        guard span > 0 else { return current.valueDb }
        let progress = min(1, max(0, (localSeconds - current.atSeconds) / span))
        let eased: Double
        switch current.easing {
        case "hold": eased = 0
        case "easeIn": eased = progress * progress
        case "easeOut": eased = 1 - pow(1 - progress, 2)
        case "easeInOut": eased = progress * progress * (3 - 2 * progress)
        default: eased = progress
        }
        return current.valueDb + (next.valueDb - current.valueDb) * eased
    }
    return clip.volumeDb
}

private func isSupportedStillImage(_ sourcePath: String) -> Bool {
    switch URL(fileURLWithPath: sourcePath).pathExtension.lowercased() {
    case "png", "jpg", "jpeg", "webp", "heic", "heif", "tif", "tiff", "bmp":
        true
    default:
        false
    }
}

private func writePreparedFrameAsset(
    _ frames: PreparedFrames,
    width: Int,
    height: Int,
    fps: Double
) async throws -> URL {
    let outputURL = FileManager.default.temporaryDirectory
        .appendingPathComponent("video-creater-prepared-\(UUID().uuidString).mov")
    let writer = try AVAssetWriter(outputURL: outputURL, fileType: .mov)
    let settings: [String: Any] = [
        AVVideoCodecKey: AVVideoCodecType.proRes4444,
        AVVideoWidthKey: width,
        AVVideoHeightKey: height,
    ]
    let input = AVAssetWriterInput(mediaType: .video, outputSettings: settings)
    input.expectsMediaDataInRealTime = false
    let attributes: [String: Any] = [
        kCVPixelBufferPixelFormatTypeKey as String: kCVPixelFormatType_32BGRA,
        kCVPixelBufferWidthKey as String: width,
        kCVPixelBufferHeightKey as String: height,
        kCVPixelBufferCGImageCompatibilityKey as String: true,
        kCVPixelBufferCGBitmapContextCompatibilityKey as String: true,
    ]
    let adaptor = AVAssetWriterInputPixelBufferAdaptor(
        assetWriterInput: input,
        sourcePixelBufferAttributes: attributes
    )
    guard writer.canAdd(input) else {
        throw ExporterError.export("AVAssetWriter rejected the prepared visual input")
    }
    writer.add(input)
    guard writer.startWriting() else {
        throw ExporterError.export("prepared visual writer could not start")
    }
    writer.startSession(atSourceTime: .zero)
    var cachedImagePath: String?
    var cachedImage: CGImage?
    for (index, path) in frames.framePaths.enumerated() {
        while !input.isReadyForMoreMediaData {
            try await Task.sleep(for: .milliseconds(2))
        }
        let image: CGImage
        if cachedImagePath == path, let cachedImage {
            image = cachedImage
        } else {
            guard let source = CGImageSourceCreateWithURL(URL(fileURLWithPath: path) as CFURL, nil),
                  let decoded = CGImageSourceCreateImageAtIndex(source, 0, nil) else {
                throw ExporterError.io("prepared PNG frame could not be decoded: \(path)")
            }
            cachedImagePath = path
            cachedImage = decoded
            image = decoded
        }
        guard let pool = adaptor.pixelBufferPool else {
            throw ExporterError.io("prepared PNG frame could not be decoded: \(path)")
        }
        var optionalBuffer: CVPixelBuffer?
        guard CVPixelBufferPoolCreatePixelBuffer(nil, pool, &optionalBuffer) == kCVReturnSuccess,
              let buffer = optionalBuffer else {
            throw ExporterError.export("prepared visual pixel buffer allocation failed")
        }
        CVPixelBufferLockBaseAddress(buffer, [])
        defer { CVPixelBufferUnlockBaseAddress(buffer, []) }
        guard let context = CGContext(
            data: CVPixelBufferGetBaseAddress(buffer),
            width: width,
            height: height,
            bitsPerComponent: 8,
            bytesPerRow: CVPixelBufferGetBytesPerRow(buffer),
            space: CGColorSpaceCreateDeviceRGB(),
            bitmapInfo: CGImageAlphaInfo.premultipliedFirst.rawValue | CGBitmapInfo.byteOrder32Little.rawValue
        ) else {
            throw ExporterError.export("prepared visual CGContext allocation failed")
        }
        context.clear(CGRect(x: 0, y: 0, width: width, height: height))
        context.draw(image, in: CGRect(x: 0, y: 0, width: width, height: height))
        let presentation = CMTime(seconds: Double(index) * frames.frameDurationSeconds, preferredTimescale: timescale)
        guard adaptor.append(buffer, withPresentationTime: presentation) else {
            throw ExporterError.export("prepared visual frame append failed")
        }
    }
    input.markAsFinished()
    await writer.finishWriting()
    guard writer.status == .completed else {
        throw ExporterError.export("prepared visual writer failed: \(writer.error?.localizedDescription ?? "unknown error")")
    }
    return outputURL
}

private struct OutputProbe {
    let durationSeconds: Double
    let sizeBytes: UInt64
    let videoCodec: String
    let audioCodec: String?
    let width: Int
    let height: Int
    let fps: Double?
}

private func probeOutput(_ outputURL: URL) async throws -> OutputProbe {
    let asset = AVURLAsset(url: outputURL)
    let duration = try await asset.load(.duration).seconds
    guard duration.isFinite, duration > 0 else {
        throw ExporterError.export("rendered output has no finite positive duration")
    }
    guard let video = try await asset.loadTracks(withMediaType: .video).first else {
        throw ExporterError.export("rendered output has no video track")
    }
    let naturalSize = try await video.load(.naturalSize)
    let preferredTransform = try await video.load(.preferredTransform)
    let displayed = CGRect(origin: .zero, size: naturalSize).applying(preferredTransform)
    let descriptions = try await video.load(.formatDescriptions)
    guard let description = descriptions.first else {
        throw ExporterError.export("rendered video track has no format description")
    }
    let videoCodec = fourCC(CMFormatDescriptionGetMediaSubType(description))
    let nominalFrameRate = try await video.load(.nominalFrameRate)
    let audioTrack = try await asset.loadTracks(withMediaType: .audio).first
    let audioCodec: String?
    if let audioTrack,
       let description = try await audioTrack.load(.formatDescriptions).first {
        audioCodec = fourCC(CMFormatDescriptionGetMediaSubType(description))
    } else {
        audioCodec = nil
    }
    let attributes = try FileManager.default.attributesOfItem(atPath: outputURL.path)
    guard let size = attributes[.size] as? NSNumber, size.uint64Value > 0 else {
        throw ExporterError.export("rendered output is empty")
    }
    return OutputProbe(
        durationSeconds: duration,
        sizeBytes: size.uint64Value,
        videoCodec: videoCodec,
        audioCodec: audioCodec,
        width: Int(abs(displayed.width).rounded()),
        height: Int(abs(displayed.height).rounded()),
        fps: nominalFrameRate > 0 ? Double(nominalFrameRate) : nil
    )
}

private func fourCC(_ code: FourCharCode) -> String {
    let bytes: [UInt8] = [
        UInt8((code >> 24) & 0xff),
        UInt8((code >> 16) & 0xff),
        UInt8((code >> 8) & 0xff),
        UInt8(code & 0xff),
    ]
    return String(bytes: bytes, encoding: .ascii) ?? String(format: "0x%08x", code)
}

private func makeLayerInstruction(
    _ prepared: PreparedVideoClip,
    renderSize: CGSize
) -> AVMutableVideoCompositionLayerInstruction {
    let clip = prepared.request
    let instruction = AVMutableVideoCompositionLayerInstruction(assetTrack: prepared.track)
    let displayedRect = CGRect(origin: .zero, size: prepared.naturalSize)
        .applying(prepared.preferredTransform)
    let displayedSize = CGSize(width: abs(displayedRect.width), height: abs(displayedRect.height))
    let croppedWidth = displayedSize.width * (1 - clip.crop.left - clip.crop.right)
    let croppedHeight = displayedSize.height * (1 - clip.crop.top - clip.crop.bottom)
    let targetWidth = renderSize.width * clip.transform.width
    let targetHeight = renderSize.height * clip.transform.height
    let scale = min(targetWidth / croppedWidth, targetHeight / croppedHeight)
    let scaledWidth = croppedWidth * scale
    let scaledHeight = croppedHeight * scale
    let targetX = renderSize.width * clip.transform.centerX - scaledWidth / 2
    let targetY = renderSize.height * clip.transform.centerY - scaledHeight / 2

    var transform = prepared.preferredTransform
        .concatenating(CGAffineTransform(
            translationX: -displayedRect.minX - displayedSize.width * clip.crop.left,
            y: -displayedRect.minY - displayedSize.height * clip.crop.top
        ))
        .concatenating(CGAffineTransform(scaleX: scale, y: scale))
    if clip.transform.flipHorizontal {
        transform = transform
            .concatenating(CGAffineTransform(translationX: scaledWidth, y: 0))
            .concatenating(CGAffineTransform(scaleX: -1, y: 1))
    }
    if clip.transform.flipVertical {
        transform = transform
            .concatenating(CGAffineTransform(translationX: 0, y: scaledHeight))
            .concatenating(CGAffineTransform(scaleX: 1, y: -1))
    }
    transform = transform.concatenating(CGAffineTransform(translationX: targetX, y: targetY))
    let start = time(clip.timelineStartSeconds)
    instruction.setTransform(transform, at: start)
    instruction.setCropRectangle(
        CGRect(x: targetX, y: targetY, width: scaledWidth, height: scaledHeight),
        at: start
    )

    let endSeconds = clip.timelineStartSeconds + clip.timelineDurationSeconds
    let fadeInEnd = clip.timelineStartSeconds + clip.fadeInSeconds
    let fadeOutStart = endSeconds - clip.fadeOutSeconds
    if clip.fadeInSeconds > 0 {
        instruction.setOpacityRamp(
            fromStartOpacity: 0,
            toEndOpacity: Float(clip.opacity),
            timeRange: CMTimeRange(start: start, duration: time(clip.fadeInSeconds))
        )
    } else {
        instruction.setOpacity(Float(clip.opacity), at: start)
    }
    if clip.fadeOutSeconds > 0 {
        instruction.setOpacityRamp(
            fromStartOpacity: Float(clip.opacity),
            toEndOpacity: 0,
            timeRange: CMTimeRange(start: time(fadeOutStart), duration: time(clip.fadeOutSeconds))
        )
    } else {
        instruction.setOpacity(Float(clip.opacity), at: time(endSeconds))
    }
    if fadeInEnd < fadeOutStart {
        instruction.setOpacity(Float(clip.opacity), at: time(fadeInEnd))
    }
    return instruction
}

private func makeAnimationTool(
    overlays: [Overlay],
    renderSize: CGSize,
    durationSeconds: Double
) throws -> AVVideoCompositionCoreAnimationTool {
    let videoLayer = CALayer()
    videoLayer.frame = CGRect(origin: .zero, size: renderSize)
    let parentLayer = CALayer()
    parentLayer.frame = videoLayer.frame
    parentLayer.isGeometryFlipped = true
    parentLayer.addSublayer(videoLayer)

    for overlay in overlays {
        let images = try overlay.framePaths.map(loadImage)
        let layer = CALayer()
        layer.frame = parentLayer.bounds
        layer.contentsGravity = .resize
        layer.opacity = 0
        layer.contents = images[0]

        let visibility = CAKeyframeAnimation(keyPath: "opacity")
        let start = overlay.timelineStartSeconds / durationSeconds
        let end = (overlay.timelineStartSeconds + overlay.durationSeconds) / durationSeconds
        visibility.values = [0, 0, 1, 1, 0, 0]
        visibility.keyTimes = [0, max(0, start - 0.000_001), start, end, min(1, end + 0.000_001), 1]
            .map(NSNumber.init(value:))
        visibility.beginTime = AVCoreAnimationBeginTimeAtZero
        visibility.duration = durationSeconds
        visibility.calculationMode = .discrete
        visibility.isRemovedOnCompletion = false
        layer.add(visibility, forKey: "visibility")

        if images.count > 1 {
            let contents = CAKeyframeAnimation(keyPath: "contents")
            contents.values = images
            contents.keyTimes = images.indices.map { index in
                NSNumber(value: Double(index) / Double(images.count))
            }
            contents.beginTime = AVCoreAnimationBeginTimeAtZero + overlay.timelineStartSeconds
            contents.duration = overlay.durationSeconds
            contents.calculationMode = .discrete
            contents.isRemovedOnCompletion = false
            layer.add(contents, forKey: "contents")
        }
        parentLayer.addSublayer(layer)
    }

    return AVVideoCompositionCoreAnimationTool(
        postProcessingAsVideoLayer: videoLayer,
        in: parentLayer
    )
}

private func loadImage(path: String) throws -> CGImage {
    let url = URL(fileURLWithPath: path) as CFURL
    guard let source = CGImageSourceCreateWithURL(url, nil),
          let image = CGImageSourceCreateImageAtIndex(source, 0, nil) else {
        throw ExporterError.io("overlay frame could not be decoded: \(path)")
    }
    return image
}

private func requestDuration(_ request: ExportRequest) -> Double {
    request.videoClips.map { $0.timelineStartSeconds + $0.timelineDurationSeconds }.max() ?? 0
}

private func time(_ seconds: Double) -> CMTime {
    CMTime(seconds: seconds, preferredTimescale: timescale)
}

private func emit(_ event: WorkerEvent) {
    emitEncodable(event)
}

private func emitEncodable(_ event: some Encodable) {
    let encoder = JSONEncoder()
    guard let data = try? encoder.encode(event) else { return }
    FileHandle.standardOutput.write(data)
    FileHandle.standardOutput.write(Data([0x0A]))
}
