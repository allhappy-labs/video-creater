import Foundation
import CryptoKit
import SpeechEnhancement

private let pinnedModelRevision = "937bad9811f1ffc1a06ea0d676461b080b2bdc93"
private let pinnedModelFiles: [(path: String, bytes: Int, sha256: String)] = [
    ("DeepFilterNet3.mlmodelc/analytics/coremldata.bin", 243, "6dc4207ced0fda3bd22928770b2a775b36cdf80938eb27074807121824a446cf"),
    ("DeepFilterNet3.mlmodelc/coremldata.bin", 415, "b1b2798fa27abb01f31871914ba956c2c098235d4af14c41336278fab70492ca"),
    ("DeepFilterNet3.mlmodelc/model.mil", 161_550, "ab6aadbc10e99866eff644385eb90b0326f0a3f57382a319f49444a31ebe1c14"),
    ("DeepFilterNet3.mlmodelc/weights/weight.bin", 2_181_056, "6b6adf3f78972bd1caed73e9e9e4eabd65590dcc9a86e41ae00e22b7c63a0018"),
    ("auxiliary.npz", 128_774, "332f9e9c4ee639e9edd61b27a873e84270b1aab1355c728b97efe7bb1d1563a1"),
    ("config.json", 36, "66c04138ec8bb287fe2cba067b31bb8cc92f7e4762fef8e8d05b082c25c1c635"),
]

private struct PCM16Wave {
    let sampleRate: Int
    let channels: Int
    let samples: [[Float]]

    init(url: URL) throws {
        let data = try Data(contentsOf: url)
        guard data.count >= 44,
              String(data: data[0..<4], encoding: .ascii) == "RIFF",
              String(data: data[8..<12], encoding: .ascii) == "WAVE"
        else { throw Failure("input must be a RIFF/WAVE file") }
        var offset = 12
        var format: (audio: UInt16, channels: UInt16, rate: UInt32, bits: UInt16)?
        var pcm: Data?
        while offset + 8 <= data.count {
            let id = String(data: data[offset..<offset + 4], encoding: .ascii) ?? ""
            let size = Int(data.u32(offset + 4))
            let start = offset + 8
            let end = min(data.count, start + size)
            guard end >= start else { break }
            if id == "fmt ", end - start >= 16 {
                format = (data.u16(start), data.u16(start + 2), data.u32(start + 4), data.u16(start + 14))
            } else if id == "data" {
                pcm = data[start..<end]
            }
            offset = end + (size & 1)
        }
        guard let format, format.audio == 1, format.channels > 0, format.bits == 16,
              let pcm else { throw Failure("input must be interleaved PCM16 WAV") }
        let channelCount = Int(format.channels)
        sampleRate = Int(format.rate)
        channels = channelCount
        let frameCount = pcm.count / (channelCount * 2)
        var decoded = [[Float]](repeating: [], count: channelCount)
        for channel in decoded.indices { decoded[channel].reserveCapacity(frameCount) }
        pcm.withUnsafeBytes { raw in
            let values = raw.bindMemory(to: Int16.self)
            for frame in 0..<frameCount {
                for channel in 0..<channelCount {
                    decoded[channel].append(Float(Int16(littleEndian: values[frame * channelCount + channel])) / 32768)
                }
            }
        }
        samples = decoded
    }

    static func write(samples: [[Float]], sampleRate: Int, url: URL) throws {
        guard let frames = samples.first?.count, frames > 0,
              samples.allSatisfy({ $0.count == frames }) else { throw Failure("enhancer returned invalid channels") }
        let channels = samples.count
        let byteCount = frames * channels * 2
        var data = Data()
        data.appendASCII("RIFF"); data.appendU32(UInt32(36 + byteCount)); data.appendASCII("WAVE")
        data.appendASCII("fmt "); data.appendU32(16); data.appendU16(1); data.appendU16(UInt16(channels))
        data.appendU32(UInt32(sampleRate)); data.appendU32(UInt32(sampleRate * channels * 2))
        data.appendU16(UInt16(channels * 2)); data.appendU16(16)
        data.appendASCII("data"); data.appendU32(UInt32(byteCount))
        for frame in 0..<frames {
            for channel in 0..<channels {
                let value = Int16((samples[channel][frame].clamped(to: -1...1) * 32767).rounded())
                data.appendU16(UInt16(bitPattern: value))
            }
        }
        try data.write(to: url, options: .atomic)
    }
}

private struct Failure: Error, CustomStringConvertible {
    let description: String
    init(_ description: String) { self.description = description }
}

@main
private enum AudioEnhanceMain {
    static func main() async {
        do {
            let arguments = try parseArguments()
            let input = try PCM16Wave(url: URL(fileURLWithPath: arguments.input))
            guard input.sampleRate == SpeechEnhancer.sampleRate else {
                throw Failure("input sample rate must be \(SpeechEnhancer.sampleRate) Hz")
            }
            let modelDirectory = URL(fileURLWithPath: arguments.modelDirectory, isDirectory: true)
                .appendingPathComponent("models", isDirectory: true)
                .appendingPathComponent("aufklarer", isDirectory: true)
                .appendingPathComponent("DeepFilterNet3-CoreML", isDirectory: true)
            try await ensurePinnedModel(at: modelDirectory)
            let enhancer = try await SpeechEnhancer.fromPretrained(
                cacheDir: modelDirectory,
                offlineMode: true
            )
            var mixed = [[Float]]()
            for dry in input.samples {
                enhancer.resetState()
                let minimumModelSamples = SpeechEnhancer.sampleRate * 30
                let modelInput = dry.count < minimumModelSamples
                    ? dry + [Float](repeating: 0, count: minimumModelSamples - dry.count)
                    : dry
                var wet = try enhancer.enhanceChunked(audio: modelInput, sampleRate: input.sampleRate)
                if wet.count < dry.count {
                    wet.append(contentsOf: repeatElement(wet.last ?? 0, count: dry.count - wet.count))
                }
                mixed.append(zip(dry, wet.prefix(dry.count)).map { drySample, wetSample in
                    drySample * Float(1 - arguments.strength) + wetSample * Float(arguments.strength)
                })
            }
            try PCM16Wave.write(
                samples: mixed,
                sampleRate: SpeechEnhancer.sampleRate,
                url: URL(fileURLWithPath: arguments.output)
            )
        } catch {
            FileHandle.standardError.write(Data("audio enhancement failed: \(error)\n".utf8))
            Foundation.exit(2)
        }
    }

    private static func ensurePinnedModel(at destination: URL) async throws {
        if verifyPinnedModel(at: destination) { return }

        let fileManager = FileManager.default
        let parent = destination.deletingLastPathComponent()
        try fileManager.createDirectory(at: parent, withIntermediateDirectories: true)
        let staging = parent.appendingPathComponent(".DeepFilterNet3-CoreML.download-\(UUID().uuidString)", isDirectory: true)
        try fileManager.createDirectory(at: staging, withIntermediateDirectories: true)
        do {
            for file in pinnedModelFiles {
                let target = staging.appendingPathComponent(file.path)
                try fileManager.createDirectory(
                    at: target.deletingLastPathComponent(),
                    withIntermediateDirectories: true
                )
                guard let url = URL(string: "https://huggingface.co/aufklarer/DeepFilterNet3-CoreML/resolve/\(pinnedModelRevision)/\(file.path)") else {
                    throw Failure("invalid pinned model URL for \(file.path)")
                }
                let (temporary, response) = try await URLSession.shared.download(from: url)
                guard let http = response as? HTTPURLResponse, http.statusCode == 200 else {
                    throw Failure("pinned model download failed for \(file.path)")
                }
                try fileManager.moveItem(at: temporary, to: target)
            }
            guard verifyPinnedModel(at: staging) else {
                throw Failure("downloaded DeepFilterNet3 model does not match pinned revision \(pinnedModelRevision)")
            }
            if fileManager.fileExists(atPath: destination.path) {
                try fileManager.removeItem(at: destination)
            }
            try fileManager.moveItem(at: staging, to: destination)
        } catch {
            try? fileManager.removeItem(at: staging)
            throw error
        }
    }

    private static func verifyPinnedModel(at directory: URL) -> Bool {
        pinnedModelFiles.allSatisfy { file in
            let url = directory.appendingPathComponent(file.path)
            guard let attributes = try? FileManager.default.attributesOfItem(atPath: url.path),
                  (attributes[.size] as? NSNumber)?.intValue == file.bytes,
                  let data = try? Data(contentsOf: url, options: .mappedIfSafe)
            else { return false }
            return SHA256.hash(data: data).map { String(format: "%02x", $0) }.joined() == file.sha256
        }
    }

    private static func parseArguments() throws -> (input: String, output: String, modelDirectory: String, strength: Double) {
        let args = CommandLine.arguments
        func value(_ flag: String) -> String? {
            guard let index = args.firstIndex(of: flag), args.indices.contains(index + 1) else { return nil }
            return args[index + 1]
        }
        guard let input = value("--input"), let output = value("--output"),
              let modelDirectory = value("--model-directory"),
              let strengthText = value("--strength"), let strength = Double(strengthText),
              (0...1).contains(strength)
        else { throw Failure("usage: --input FILE --output FILE --model-directory DIR --strength 0...1") }
        return (input, output, modelDirectory, strength)
    }
}

private extension Data {
    func u16(_ offset: Int) -> UInt16 { withUnsafeBytes { $0.loadUnaligned(fromByteOffset: offset, as: UInt16.self).littleEndian } }
    func u32(_ offset: Int) -> UInt32 { withUnsafeBytes { $0.loadUnaligned(fromByteOffset: offset, as: UInt32.self).littleEndian } }
    mutating func appendASCII(_ value: String) { append(value.data(using: .ascii)!) }
    mutating func appendU16(_ value: UInt16) { var value = value.littleEndian; append(Data(bytes: &value, count: 2)) }
    mutating func appendU32(_ value: UInt32) { var value = value.littleEndian; append(Data(bytes: &value, count: 4)) }
}

private extension Float {
    func clamped(to range: ClosedRange<Float>) -> Float { min(range.upperBound, max(range.lowerBound, self)) }
}
