import Foundation
import VideoCreaterFluidAudioTranscribeCore

@main
struct VideoCreaterFluidAudioTranscribe {
    static func main() async {
        do {
            let input = FileHandle.standardInput.readDataToEndOfFile()
            switch try decodeHelperRequest(input) {
            case .transcription(let request):
                try writeJSON(try await transcribe(request))
            case .speechAnalysis(let request):
                try writeJSON(try await analyzeSpeech(request))
            }
            Foundation.exit(0)
        } catch {
            let response = ErrorResponse(
                schemaVersion: 1,
                error: (error as? LocalizedError)?.errorDescription ?? String(describing: error)
            )
            try? writeJSON(response)
            Foundation.exit(1)
        }
    }

    private static func writeJSON<T: Encodable>(_ value: T) throws {
        let encoder = JSONEncoder()
        encoder.outputFormatting = [.sortedKeys]
        var data = try encoder.encode(value)
        data.append(0x0A)
        try FileHandle.standardOutput.write(contentsOf: data)
    }
}
