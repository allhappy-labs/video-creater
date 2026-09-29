import Foundation
import FluidAudio
import XCTest
@testable import VideoCreaterFluidAudioTranscribeCore

final class ProtocolTests: XCTestCase {
    func testResponseEncodesRuntimeAndWords() throws {
        let response = TranscriptionResponse(
            schemaVersion: 1,
            runtimeId: "fluid_audio_coreml",
            modelId: "nvidia/parakeet-tdt-0.6b-v3",
            text: "hello world",
            durationSeconds: 1.5,
            processingSeconds: 0.25,
            words: [
                Word(
                    text: "hello",
                    startSeconds: 0.1,
                    endSeconds: 0.4,
                    confidence: 0.98
                )
            ]
        )

        let encoded = try JSONEncoder().encode(response)
        let json = try JSONSerialization.jsonObject(with: encoded) as? [String: Any]

        XCTAssertEqual(json?["runtimeId"] as? String, "fluid_audio_coreml")
        let words = try XCTUnwrap(json?["words"] as? [[String: Any]])
        XCTAssertEqual(words.first?["text"] as? String, "hello")
        XCTAssertEqual(words.first?["startSeconds"] as? Double, 0.1)
        XCTAssertEqual(words.first?["endSeconds"] as? Double, 0.4)
        XCTAssertEqual(words.first?["confidence"] as? Double, 0.98)
    }

    func testRequestRoundTripsModelPathWithoutAppendingSubdirectories() throws {
        let request = TranscriptionRequest(
            schemaVersion: 1,
            modelId: "nvidia/parakeet-tdt-0.6b-v3",
            modelPath: "/Users/test/Models/nvidia-parakeet/fluid-audio-coreml",
            mediaPath: "/Users/test/input.mov",
            languageMode: "en"
        )

        let encoded = try JSONEncoder().encode(request)
        let decoded = try JSONDecoder().decode(TranscriptionRequest.self, from: encoded)

        XCTAssertEqual(decoded.modelPath, request.modelPath)
        XCTAssertFalse(decoded.modelPath.contains("parakeet-tdt-0.6b-v3-coreml"))
    }

    func testVocabularyLoaderPrefersCatalogV3VocabularyFilename() throws {
        let directory = FileManager.default.temporaryDirectory
            .appendingPathComponent("VideoCreaterFluidAudioTranscribeTests-\(UUID().uuidString)", isDirectory: true)
        try FileManager.default.createDirectory(at: directory, withIntermediateDirectories: true)
        defer {
            try? FileManager.default.removeItem(at: directory)
        }

        try Data(#"["legacy"]"#.utf8).write(to: directory.appendingPathComponent("parakeet_vocab.json"))
        try Data(#"["catalog-v3"]"#.utf8).write(to: directory.appendingPathComponent("parakeet_v3_vocab.json"))

        let vocabulary = try loadParakeetVocabulary(from: directory)

        XCTAssertEqual(vocabulary[0], "catalog-v3")
    }

    func testLanguageHintTreatsAutoAndBlankAsNil() {
        XCTAssertNil(languageHint(nil))
        XCTAssertNil(languageHint(""))
        XCTAssertNil(languageHint("   "))
        XCTAssertNil(languageHint("auto"))
        XCTAssertNil(languageHint("AUTO"))
    }

    func testHelperDecoderPreservesLegacyTranscriptionProtocol() throws {
        let data = Data(
            #"{"schemaVersion":1,"modelId":"nvidia/parakeet-tdt-0.6b-v3","modelPath":"/models/parakeet","mediaPath":"/media/input.wav","languageMode":"auto"}"#.utf8
        )

        guard case .transcription(let request) = try decodeHelperRequest(data) else {
            return XCTFail("Expected the legacy request to remain a transcription request")
        }
        XCTAssertEqual(request.modelId, "nvidia/parakeet-tdt-0.6b-v3")
        XCTAssertEqual(request.languageMode, "auto")
    }

    func testHelperDecoderSelectsExplicitSpeechAnalysisMode() throws {
        let request = speechRequest()
        let data = try JSONEncoder().encode(request)

        guard case .speechAnalysis(let decoded) = try decodeHelperRequest(data) else {
            return XCTFail("Expected speech-analysis dispatch")
        }
        XCTAssertEqual(decoded, request)
    }

    func testSpeechAnalysisResponseCarriesValidatedTimingEmbeddingsAndMetadata() throws {
        let response = try validatedSpeechAnalysisResponse(
            request: speechRequest(),
            durationSeconds: 0.3,
            processingSeconds: 0.2,
            vadResults: [
                VadResult(
                    probability: 0.75,
                    isVoiceActive: true,
                    processingTime: 0.01,
                    outputState: .initial()
                ),
                VadResult(
                    probability: 0.25,
                    isVoiceActive: false,
                    processingTime: 0.01,
                    outputState: .initial()
                ),
            ],
            diarizationSegments: [
                TimedSpeakerSegment(
                    speakerId: "S1",
                    embedding: Array(repeating: 0.0625, count: 256),
                    startTimeSeconds: 0,
                    endTimeSeconds: 0.3,
                    qualityScore: 0.9
                )
            ]
        )

        XCTAssertEqual(response.runtimeId, "fluid_audio_speech_analysis")
        XCTAssertEqual(response.sampleRate, 16_000)
        XCTAssertEqual(response.vadSegments.count, 2)
        XCTAssertEqual(response.vadSegments[1].endSeconds, 0.3, accuracy: 0.000_001)
        XCTAssertEqual(response.diarizationSegments[0].embedding.count, 256)
        XCTAssertEqual(response.vadModel.revision, "silero-revision")
        XCTAssertEqual(response.diarizationModel.revision, "diarizer-revision")
        XCTAssertEqual(response.diarizationModel.embeddingDimensions, 256)

        let data = try JSONEncoder().encode(response)
        let decoded = try JSONDecoder().decode(SpeechAnalysisResponse.self, from: data)
        XCTAssertEqual(decoded, response)
    }

    func testSpeechAnalysisResponseRejectsUnexpectedEmbeddingDimensions() {
        XCTAssertThrowsError(
            try validatedSpeechAnalysisResponse(
                request: speechRequest(),
                durationSeconds: 1,
                processingSeconds: 0.1,
                vadResults: [],
                diarizationSegments: [
                    TimedSpeakerSegment(
                        speakerId: "S1",
                        embedding: [1, 0, 0],
                        startTimeSeconds: 0,
                        endTimeSeconds: 1,
                        qualityScore: 1
                    )
                ]
            )
        ) { error in
            XCTAssertEqual(
                error.localizedDescription,
                "Offline diarization embedding must contain 256 values; found 3"
            )
        }
    }

    func testHelperDecoderRejectsUnknownMode() throws {
        let data = Data(#"{"schemaVersion":1,"mode":"remoteSpeech"}"#.utf8)
        XCTAssertThrowsError(try decodeHelperRequest(data)) { error in
            XCTAssertEqual(error.localizedDescription, "Unsupported helper request mode 'remoteSpeech'")
        }
    }

    private func speechRequest() -> SpeechAnalysisRequest {
        SpeechAnalysisRequest(
            schemaVersion: 1,
            mediaPath: "/media/input.wav",
            vadModelId: "silero-vad-v6",
            vadModelRevision: "silero-revision",
            vadModelRoot: "/models/vad",
            diarizationModelId: "fluid-offline-diarizer",
            diarizationModelRevision: "diarizer-revision",
            diarizationModelRoot: "/models/diarizer"
        )
    }
}
