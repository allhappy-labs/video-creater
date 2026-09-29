import Foundation
@preconcurrency import CoreML
import FluidAudio

public let fluidAudioCoreMLRuntimeId = "fluid_audio_coreml"

public struct TranscriptionRequest: Codable, Equatable, Sendable {
    public let schemaVersion: Int
    public let modelId: String
    public let modelPath: String
    public let mediaPath: String
    public let languageMode: String?

    public init(
        schemaVersion: Int,
        modelId: String,
        modelPath: String,
        mediaPath: String,
        languageMode: String?
    ) {
        self.schemaVersion = schemaVersion
        self.modelId = modelId
        self.modelPath = modelPath
        self.mediaPath = mediaPath
        self.languageMode = languageMode
    }
}

public struct Word: Codable, Equatable, Sendable {
    public let text: String
    public let startSeconds: Double
    public let endSeconds: Double
    public let confidence: Double

    public init(
        text: String,
        startSeconds: Double,
        endSeconds: Double,
        confidence: Double
    ) {
        self.text = text
        self.startSeconds = startSeconds
        self.endSeconds = endSeconds
        self.confidence = confidence
    }
}

public struct TranscriptionResponse: Codable, Equatable, Sendable {
    public let schemaVersion: Int
    public let runtimeId: String
    public let modelId: String
    public let text: String
    public let durationSeconds: Double
    public let processingSeconds: Double
    public let words: [Word]

    public init(
        schemaVersion: Int,
        runtimeId: String,
        modelId: String,
        text: String,
        durationSeconds: Double,
        processingSeconds: Double,
        words: [Word]
    ) {
        self.schemaVersion = schemaVersion
        self.runtimeId = runtimeId
        self.modelId = modelId
        self.text = text
        self.durationSeconds = durationSeconds
        self.processingSeconds = processingSeconds
        self.words = words
    }
}

public struct ErrorResponse: Codable, Equatable, Sendable {
    public let schemaVersion: Int
    public let error: String

    public init(schemaVersion: Int, error: String) {
        self.schemaVersion = schemaVersion
        self.error = error
    }
}

public enum TranscriptionError: LocalizedError, Sendable {
    case unsupportedSchemaVersion(Int)
    case missingModelDirectory(String)
    case missingModelFile(String, String)
    case missingVocabulary(String)
    case invalidVocabulary(String)
    case missingMediaFile(String)
    case unsupportedLanguage(String)

    public var errorDescription: String? {
        switch self {
        case .unsupportedSchemaVersion(let version):
            return "Unsupported transcription request schemaVersion \(version); expected 1"
        case .missingModelDirectory(let path):
            return "Model directory not found at \(path)"
        case .missingModelFile(let name, let path):
            return "Required Core ML model \(name) not found at \(path)"
        case .missingVocabulary(let path):
            return "Parakeet vocabulary not found at \(path)"
        case .invalidVocabulary(let path):
            return "Parakeet vocabulary has an unsupported JSON shape at \(path)"
        case .missingMediaFile(let path):
            return "Media file not found at \(path)"
        case .unsupportedLanguage(let language):
            return "Unsupported FluidAudio language hint '\(language)'"
        }
    }
}

public func languageHint(_ mode: String?) -> Language? {
    guard let mode else { return nil }
    let normalized = mode.trimmingCharacters(in: .whitespacesAndNewlines).lowercased()
    guard !normalized.isEmpty, normalized != "auto" else { return nil }
    return Language(rawValue: normalized)
}

public func transcribe(_ request: TranscriptionRequest) async throws -> TranscriptionResponse {
    guard request.schemaVersion == 1 else {
        throw TranscriptionError.unsupportedSchemaVersion(request.schemaVersion)
    }

    let modelDirectory = URL(fileURLWithPath: request.modelPath, isDirectory: true)
    var isDirectory: ObjCBool = false
    guard FileManager.default.fileExists(atPath: modelDirectory.path, isDirectory: &isDirectory),
        isDirectory.boolValue
    else {
        throw TranscriptionError.missingModelDirectory(request.modelPath)
    }

    let mediaURL = URL(fileURLWithPath: request.mediaPath)
    guard FileManager.default.fileExists(atPath: mediaURL.path) else {
        throw TranscriptionError.missingMediaFile(request.mediaPath)
    }

    let language = try resolvedLanguageHint(request.languageMode)
    let models = try loadLocalV3Models(from: modelDirectory)
    let manager = AsrManager(
        config: ASRConfig(melChunkContext: false),
        models: models
    )
    var decoderState = try TdtDecoderState(decoderLayers: await manager.decoderLayerCount)
    let result = try await manager.transcribe(mediaURL, decoderState: &decoderState, language: language)

    return TranscriptionResponse(
        schemaVersion: 1,
        runtimeId: fluidAudioCoreMLRuntimeId,
        modelId: request.modelId,
        text: result.text,
        durationSeconds: result.duration,
        processingSeconds: result.processingTime,
        words: words(from: result.tokenTimings)
    )
}

private func resolvedLanguageHint(_ mode: String?) throws -> Language? {
    let normalized = mode?.trimmingCharacters(in: .whitespacesAndNewlines).lowercased() ?? ""
    if normalized.isEmpty || normalized == "auto" {
        return nil
    }
    guard let language = languageHint(normalized) else {
        throw TranscriptionError.unsupportedLanguage(normalized)
    }
    return language
}

private func loadLocalV3Models(from directory: URL) throws -> AsrModels {
    let fileManager = FileManager.default
    let requiredModelFiles = [
        "Preprocessor.mlmodelc",
        "Encoder.mlmodelc",
        "Decoder.mlmodelc",
        "JointDecisionv3.mlmodelc",
    ]

    for fileName in requiredModelFiles {
        let modelURL = directory.appendingPathComponent(fileName)
        var isDirectory: ObjCBool = false
        guard fileManager.fileExists(atPath: modelURL.path, isDirectory: &isDirectory),
            isDirectory.boolValue
        else {
            throw TranscriptionError.missingModelFile(fileName, modelURL.path)
        }
    }

    let baseConfig = MLModelConfiguration()
    baseConfig.computeUnits = ProcessInfo.processInfo.environment["CI"] == nil
        ? .cpuAndNeuralEngine
        : .cpuOnly

    let preprocessorConfig = MLModelConfiguration()
    preprocessorConfig.computeUnits = .cpuOnly

    let preprocessor = try MLModel(
        contentsOf: directory.appendingPathComponent("Preprocessor.mlmodelc"),
        configuration: preprocessorConfig
    )
    let encoder = try MLModel(
        contentsOf: directory.appendingPathComponent("Encoder.mlmodelc"),
        configuration: baseConfig
    )
    let decoder = try MLModel(
        contentsOf: directory.appendingPathComponent("Decoder.mlmodelc"),
        configuration: baseConfig
    )
    let joint = try MLModel(
        contentsOf: directory.appendingPathComponent("JointDecisionv3.mlmodelc"),
        configuration: baseConfig
    )

    return AsrModels(
        encoder: encoder,
        preprocessor: preprocessor,
        decoder: decoder,
        joint: joint,
        configuration: baseConfig,
        vocabulary: try loadParakeetVocabulary(from: directory),
        version: .v3
    )
}

func loadParakeetVocabulary(from directory: URL) throws -> [Int: String] {
    let vocabularyFileNames = [
        "parakeet_v3_vocab.json",
        "parakeet_vocab.json",
    ]
    guard let vocabularyURL = vocabularyFileNames
        .map({ directory.appendingPathComponent($0) })
        .first(where: { FileManager.default.fileExists(atPath: $0.path) })
    else {
        throw TranscriptionError.missingVocabulary(
            directory.appendingPathComponent(vocabularyFileNames[0]).path
        )
    }

    let data = try Data(contentsOf: vocabularyURL)
    let json = try JSONSerialization.jsonObject(with: data)

    if let array = json as? [String] {
        return Dictionary(uniqueKeysWithValues: array.enumerated().map { ($0.offset, $0.element) })
    }

    if let dictionary = json as? [String: String] {
        return Dictionary(uniqueKeysWithValues: dictionary.compactMap { key, value in
            guard let id = Int(key) else { return nil }
            return (id, value)
        })
    }

    throw TranscriptionError.invalidVocabulary(vocabularyURL.path)
}

private func words(from tokenTimings: [TokenTiming]?) -> [Word] {
    guard let tokenTimings, !tokenTimings.isEmpty else { return [] }

    var words: [Word] = []
    var currentText = ""
    var currentStart = 0.0
    var currentEnd = 0.0
    var confidenceSum = 0.0
    var confidenceCount = 0

    func finishCurrentWord() {
        let text = currentText.trimmingCharacters(in: .whitespacesAndNewlines)
        guard !text.isEmpty else { return }
        let confidence = confidenceCount > 0 ? confidenceSum / Double(confidenceCount) : 0.0
        words.append(
            Word(
                text: text,
                startSeconds: currentStart,
                endSeconds: currentEnd,
                confidence: max(0.0, min(1.0, confidence))
            )
        )
    }

    for timing in tokenTimings {
        let rawToken = timing.token
        let startsNewWord = rawToken.contains("\u{2581}") || rawToken.hasPrefix(" ")
        let normalized = rawToken
            .replacingOccurrences(of: "\u{2581}", with: " ")
            .trimmingCharacters(in: .whitespacesAndNewlines)

        guard !normalized.isEmpty else { continue }

        if startsNewWord && !currentText.trimmingCharacters(in: .whitespacesAndNewlines).isEmpty {
            finishCurrentWord()
            currentText = ""
            confidenceSum = 0.0
            confidenceCount = 0
        }

        if currentText.isEmpty {
            currentStart = timing.startTime
        }

        currentText += normalized
        currentEnd = timing.endTime
        confidenceSum += Double(timing.confidence)
        confidenceCount += 1
    }

    finishCurrentWord()
    return words
}
