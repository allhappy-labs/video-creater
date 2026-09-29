import Foundation
@preconcurrency import CoreML
import FluidAudio

public let fluidAudioSpeechAnalysisRuntimeId = "fluid_audio_speech_analysis"
public let speechAnalysisMode = "speechAnalysis"
public let speechAnalysisEmbeddingDimensions = 256

public struct SpeechAnalysisRequest: Codable, Equatable, Sendable {
    public let schemaVersion: Int
    public let mode: String
    public let mediaPath: String
    public let vadModelId: String
    public let vadModelRevision: String
    public let vadModelRoot: String
    public let diarizationModelId: String
    public let diarizationModelRevision: String
    public let diarizationModelRoot: String

    public init(
        schemaVersion: Int,
        mode: String = speechAnalysisMode,
        mediaPath: String,
        vadModelId: String,
        vadModelRevision: String,
        vadModelRoot: String,
        diarizationModelId: String,
        diarizationModelRevision: String,
        diarizationModelRoot: String
    ) {
        self.schemaVersion = schemaVersion
        self.mode = mode
        self.mediaPath = mediaPath
        self.vadModelId = vadModelId
        self.vadModelRevision = vadModelRevision
        self.vadModelRoot = vadModelRoot
        self.diarizationModelId = diarizationModelId
        self.diarizationModelRevision = diarizationModelRevision
        self.diarizationModelRoot = diarizationModelRoot
    }
}

public enum HelperRequest: Equatable, Sendable {
    case transcription(TranscriptionRequest)
    case speechAnalysis(SpeechAnalysisRequest)
}

public struct SpeechAnalysisModelMetadata: Codable, Equatable, Sendable {
    public let id: String
    public let revision: String
    public let artifactFormat: String
    public let embeddingDimensions: Int?

    public init(
        id: String,
        revision: String,
        artifactFormat: String = "coreml_compiled_bundle",
        embeddingDimensions: Int? = nil
    ) {
        self.id = id
        self.revision = revision
        self.artifactFormat = artifactFormat
        self.embeddingDimensions = embeddingDimensions
    }
}

public struct VadProbabilitySegment: Codable, Equatable, Sendable {
    public let startSeconds: Double
    public let endSeconds: Double
    public let speechProbability: Double

    public init(startSeconds: Double, endSeconds: Double, speechProbability: Double) {
        self.startSeconds = startSeconds
        self.endSeconds = endSeconds
        self.speechProbability = speechProbability
    }
}

public struct DiarizationEmbeddingSegment: Codable, Equatable, Sendable {
    public let startSeconds: Double
    public let endSeconds: Double
    public let speakerId: String
    public let confidence: Double
    public let embedding: [Double]

    public init(
        startSeconds: Double,
        endSeconds: Double,
        speakerId: String,
        confidence: Double,
        embedding: [Double]
    ) {
        self.startSeconds = startSeconds
        self.endSeconds = endSeconds
        self.speakerId = speakerId
        self.confidence = confidence
        self.embedding = embedding
    }
}

public struct SpeechAnalysisResponse: Codable, Equatable, Sendable {
    public let schemaVersion: Int
    public let mode: String
    public let runtimeId: String
    public let sampleRate: Int
    public let durationSeconds: Double
    public let processingSeconds: Double
    public let vadModel: SpeechAnalysisModelMetadata
    public let diarizationModel: SpeechAnalysisModelMetadata
    public let vadSegments: [VadProbabilitySegment]
    public let diarizationSegments: [DiarizationEmbeddingSegment]

    public init(
        schemaVersion: Int = 1,
        mode: String = speechAnalysisMode,
        runtimeId: String = fluidAudioSpeechAnalysisRuntimeId,
        sampleRate: Int,
        durationSeconds: Double,
        processingSeconds: Double,
        vadModel: SpeechAnalysisModelMetadata,
        diarizationModel: SpeechAnalysisModelMetadata,
        vadSegments: [VadProbabilitySegment],
        diarizationSegments: [DiarizationEmbeddingSegment]
    ) {
        self.schemaVersion = schemaVersion
        self.mode = mode
        self.runtimeId = runtimeId
        self.sampleRate = sampleRate
        self.durationSeconds = durationSeconds
        self.processingSeconds = processingSeconds
        self.vadModel = vadModel
        self.diarizationModel = diarizationModel
        self.vadSegments = vadSegments
        self.diarizationSegments = diarizationSegments
    }
}

public enum SpeechAnalysisError: LocalizedError, Sendable {
    case unsupportedMode(String)
    case unsupportedSchemaVersion(Int)
    case missingRequestValue(String)
    case missingModelRoot(String)
    case missingModelFile(String, String)
    case invalidModelParameters(String)
    case missingMediaFile(String)
    case invalidDuration(Double)
    case invalidSegment(String)
    case invalidEmbeddingDimensions(Int)

    public var errorDescription: String? {
        switch self {
        case .unsupportedMode(let mode):
            return "Unsupported helper request mode '\(mode)'"
        case .unsupportedSchemaVersion(let version):
            return "Unsupported speech-analysis request schemaVersion \(version); expected 1"
        case .missingRequestValue(let name):
            return "Speech-analysis request value '\(name)' must not be blank"
        case .missingModelRoot(let path):
            return "Preinstalled speech model root not found at \(path)"
        case .missingModelFile(let name, let root):
            return "Required preinstalled speech model \(name) not found under \(root)"
        case .invalidModelParameters(let path):
            return "Offline diarization PLDA parameters are invalid at \(path)"
        case .missingMediaFile(let path):
            return "Media file not found at \(path)"
        case .invalidDuration(let duration):
            return "Speech-analysis duration is invalid: \(duration)"
        case .invalidSegment(let message):
            return "Speech-analysis segment is invalid: \(message)"
        case .invalidEmbeddingDimensions(let count):
            return "Offline diarization embedding must contain \(speechAnalysisEmbeddingDimensions) values; found \(count)"
        }
    }
}

private struct RequestModeProbe: Decodable {
    let mode: String?
}

public func decodeHelperRequest(_ data: Data) throws -> HelperRequest {
    let decoder = JSONDecoder()
    let probe = try decoder.decode(RequestModeProbe.self, from: data)
    switch probe.mode {
    case nil, "", "transcription":
        return .transcription(try decoder.decode(TranscriptionRequest.self, from: data))
    case speechAnalysisMode:
        return .speechAnalysis(try decoder.decode(SpeechAnalysisRequest.self, from: data))
    case .some(let mode):
        throw SpeechAnalysisError.unsupportedMode(mode)
    }
}

public func analyzeSpeech(_ request: SpeechAnalysisRequest) async throws -> SpeechAnalysisResponse {
    try validateRequest(request)

    let mediaURL = URL(fileURLWithPath: request.mediaPath)
    guard FileManager.default.fileExists(atPath: mediaURL.path) else {
        throw SpeechAnalysisError.missingMediaFile(request.mediaPath)
    }

    let startedAt = Date()
    let computeUnits: MLComputeUnits = ProcessInfo.processInfo.environment["CI"] == nil
        ? .all
        : .cpuOnly
    let configuration = MLModelConfiguration()
    configuration.computeUnits = computeUnits

    let vadModelURL = try locateCompiledModel(
        named: ModelNames.VAD.sileroVadFile,
        rootPath: request.vadModelRoot,
        repoFolderName: Repo.vad.folderName
    )
    let vadModel = try MLModel(contentsOf: vadModelURL, configuration: configuration)
    let vadManager = VadManager(config: .default, vadModel: vadModel)

    let audio = try AudioConverter().resampleAudioFile(mediaURL)
    let durationSeconds = Double(audio.count) / Double(VadManager.sampleRate)
    guard durationSeconds.isFinite, durationSeconds > 0 else {
        throw SpeechAnalysisError.invalidDuration(durationSeconds)
    }
    let vadResults = try await vadManager.process(audio)

    let diarizationModels = try loadOfflineDiarizationModels(
        rootPath: request.diarizationModelRoot,
        configuration: configuration
    )
    var diarizerConfig = OfflineDiarizerConfig.default
    diarizerConfig.exposeChunkEmbeddings = true
    let diarizer = OfflineDiarizerManager(config: diarizerConfig)
    diarizer.initialize(models: diarizationModels)

    let diarizationResult: DiarizationResult?
    do {
        diarizationResult = try await diarizer.process(audio: audio)
    } catch OfflineDiarizationError.noSpeechDetected {
        diarizationResult = nil
    }

    return try validatedSpeechAnalysisResponse(
        request: request,
        durationSeconds: durationSeconds,
        processingSeconds: Date().timeIntervalSince(startedAt),
        vadResults: vadResults,
        diarizationSegments: diarizationResult?.segments ?? []
    )
}

public func validatedSpeechAnalysisResponse(
    request: SpeechAnalysisRequest,
    durationSeconds: Double,
    processingSeconds: Double,
    vadResults: [VadResult],
    diarizationSegments: [TimedSpeakerSegment]
) throws -> SpeechAnalysisResponse {
    guard durationSeconds.isFinite, durationSeconds > 0 else {
        throw SpeechAnalysisError.invalidDuration(durationSeconds)
    }

    let chunkDuration = Double(VadManager.chunkSize) / Double(VadManager.sampleRate)
    let vadSegments: [VadProbabilitySegment] = try vadResults.enumerated().compactMap {
        index, result -> VadProbabilitySegment? in
        let start = Double(index) * chunkDuration
        guard start < durationSeconds else { return nil }
        let end = min(durationSeconds, start + chunkDuration)
        let probability = Double(result.probability)
        guard probability.isFinite, probability >= 0, probability <= 1, end > start else {
            throw SpeechAnalysisError.invalidSegment("VAD window \(index) has invalid timing or probability")
        }
        return VadProbabilitySegment(
            startSeconds: start,
            endSeconds: end,
            speechProbability: probability
        )
    }

    let embeddedSegments = try diarizationSegments.enumerated().map { index, segment in
        let start = Double(segment.startTimeSeconds)
        let unclampedEnd = Double(segment.endTimeSeconds)
        let end = min(durationSeconds, unclampedEnd)
        let confidence = Double(segment.qualityScore)
        let embedding = segment.embedding.map(Double.init)
        guard start.isFinite, end.isFinite, start >= 0, end > start, start < durationSeconds else {
            throw SpeechAnalysisError.invalidSegment("diarization window \(index) has invalid timing")
        }
        guard confidence.isFinite, confidence >= 0, confidence <= 1 else {
            throw SpeechAnalysisError.invalidSegment("diarization window \(index) has invalid confidence")
        }
        guard embedding.count == speechAnalysisEmbeddingDimensions else {
            throw SpeechAnalysisError.invalidEmbeddingDimensions(embedding.count)
        }
        guard embedding.allSatisfy(\.isFinite) else {
            throw SpeechAnalysisError.invalidSegment("diarization window \(index) has a non-finite embedding")
        }
        let speakerId = segment.speakerId.trimmingCharacters(in: .whitespacesAndNewlines)
        guard !speakerId.isEmpty else {
            throw SpeechAnalysisError.invalidSegment("diarization window \(index) has a blank speaker id")
        }
        return DiarizationEmbeddingSegment(
            startSeconds: start,
            endSeconds: end,
            speakerId: speakerId,
            confidence: confidence,
            embedding: embedding
        )
    }

    return SpeechAnalysisResponse(
        sampleRate: VadManager.sampleRate,
        durationSeconds: durationSeconds,
        processingSeconds: max(0, processingSeconds.isFinite ? processingSeconds : 0),
        vadModel: SpeechAnalysisModelMetadata(
            id: request.vadModelId,
            revision: request.vadModelRevision
        ),
        diarizationModel: SpeechAnalysisModelMetadata(
            id: request.diarizationModelId,
            revision: request.diarizationModelRevision,
            embeddingDimensions: speechAnalysisEmbeddingDimensions
        ),
        vadSegments: vadSegments,
        diarizationSegments: embeddedSegments
    )
}

private func validateRequest(_ request: SpeechAnalysisRequest) throws {
    guard request.schemaVersion == 1 else {
        throw SpeechAnalysisError.unsupportedSchemaVersion(request.schemaVersion)
    }
    guard request.mode == speechAnalysisMode else {
        throw SpeechAnalysisError.unsupportedMode(request.mode)
    }
    for (name, value) in [
        ("mediaPath", request.mediaPath),
        ("vadModelId", request.vadModelId),
        ("vadModelRevision", request.vadModelRevision),
        ("vadModelRoot", request.vadModelRoot),
        ("diarizationModelId", request.diarizationModelId),
        ("diarizationModelRevision", request.diarizationModelRevision),
        ("diarizationModelRoot", request.diarizationModelRoot),
    ] where value.trimmingCharacters(in: .whitespacesAndNewlines).isEmpty {
        throw SpeechAnalysisError.missingRequestValue(name)
    }
}

private func locateCompiledModel(
    named name: String,
    rootPath: String,
    repoFolderName: String
) throws -> URL {
    let root = URL(fileURLWithPath: rootPath, isDirectory: true).standardizedFileURL
    var isDirectory: ObjCBool = false
    guard FileManager.default.fileExists(atPath: root.path, isDirectory: &isDirectory),
        isDirectory.boolValue
    else {
        throw SpeechAnalysisError.missingModelRoot(rootPath)
    }

    let candidates = [
        root,
        root.appendingPathComponent(name, isDirectory: true),
        root.appendingPathComponent(repoFolderName, isDirectory: true)
            .appendingPathComponent(name, isDirectory: true),
        root.appendingPathComponent("\(repoFolderName)-coreml", isDirectory: true)
            .appendingPathComponent(name, isDirectory: true),
        root.appendingPathComponent("Models", isDirectory: true)
            .appendingPathComponent(repoFolderName, isDirectory: true)
            .appendingPathComponent(name, isDirectory: true),
    ]
    for candidate in candidates where candidate.lastPathComponent == name {
        var candidateIsDirectory: ObjCBool = false
        if FileManager.default.fileExists(atPath: candidate.path, isDirectory: &candidateIsDirectory),
            candidateIsDirectory.boolValue
        {
            return candidate
        }
    }
    throw SpeechAnalysisError.missingModelFile(name, root.path)
}

private func loadOfflineDiarizationModels(
    rootPath: String,
    configuration: MLModelConfiguration
) throws -> OfflineDiarizerModels {
    let startedAt = Date()
    let repo = Repo.diarizer.folderName
    let segmentationURL = try locateCompiledModel(
        named: ModelNames.OfflineDiarizer.segmentationPath,
        rootPath: rootPath,
        repoFolderName: repo
    )
    let fbankURL = try locateCompiledModel(
        named: ModelNames.OfflineDiarizer.fbankPath,
        rootPath: rootPath,
        repoFolderName: repo
    )
    let embeddingURL = try locateCompiledModel(
        named: ModelNames.OfflineDiarizer.embeddingPath,
        rootPath: rootPath,
        repoFolderName: repo
    )
    let pldaURL = try locateCompiledModel(
        named: ModelNames.OfflineDiarizer.pldaRhoPath,
        rootPath: rootPath,
        repoFolderName: repo
    )
    let parametersURL = try locateFile(
        named: ModelNames.OfflineDiarizer.pldaParameters,
        rootPath: rootPath,
        repoFolderName: repo
    )

    let fbankConfiguration = MLModelConfiguration()
    fbankConfiguration.computeUnits = .cpuOnly
    return OfflineDiarizerModels(
        segmentationModel: try MLModel(contentsOf: segmentationURL, configuration: configuration),
        fbankModel: try MLModel(contentsOf: fbankURL, configuration: fbankConfiguration),
        embeddingModel: try MLModel(contentsOf: embeddingURL, configuration: configuration),
        pldaRhoModel: try MLModel(contentsOf: pldaURL, configuration: configuration),
        pldaPsi: try loadPldaPsi(from: parametersURL),
        compilationDuration: Date().timeIntervalSince(startedAt)
    )
}

private func locateFile(named name: String, rootPath: String, repoFolderName: String) throws -> URL {
    let root = URL(fileURLWithPath: rootPath, isDirectory: true).standardizedFileURL
    var isDirectory: ObjCBool = false
    guard FileManager.default.fileExists(atPath: root.path, isDirectory: &isDirectory),
        isDirectory.boolValue
    else {
        throw SpeechAnalysisError.missingModelRoot(rootPath)
    }
    let candidates = [
        root.appendingPathComponent(name),
        root.appendingPathComponent(repoFolderName, isDirectory: true).appendingPathComponent(name),
        root.appendingPathComponent("\(repoFolderName)-coreml", isDirectory: true).appendingPathComponent(name),
        root.appendingPathComponent("Models", isDirectory: true)
            .appendingPathComponent(repoFolderName, isDirectory: true)
            .appendingPathComponent(name),
    ]
    if let candidate = candidates.first(where: { FileManager.default.fileExists(atPath: $0.path) }) {
        return candidate
    }
    throw SpeechAnalysisError.missingModelFile(name, root.path)
}

private func loadPldaPsi(from url: URL) throws -> [Double] {
    let data = try Data(contentsOf: url)
    guard
        let root = try JSONSerialization.jsonObject(with: data) as? [String: Any],
        let tensors = root["tensors"] as? [String: Any],
        let psi = tensors["psi"] as? [String: Any],
        let encoded = psi["data_base64"] as? String,
        let decoded = Data(base64Encoded: encoded, options: [.ignoreUnknownCharacters]),
        !decoded.isEmpty,
        decoded.count.isMultiple(of: MemoryLayout<Float>.size)
    else {
        throw SpeechAnalysisError.invalidModelParameters(url.path)
    }
    var values = [Float](repeating: 0, count: decoded.count / MemoryLayout<Float>.size)
    _ = values.withUnsafeMutableBytes { decoded.copyBytes(to: $0) }
    guard values.allSatisfy(\.isFinite) else {
        throw SpeechAnalysisError.invalidModelParameters(url.path)
    }
    return values.map(Double.init)
}
