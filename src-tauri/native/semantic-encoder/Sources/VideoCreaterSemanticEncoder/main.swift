import CoreGraphics
import CoreML
import CoreVideo
import Foundation
import ImageIO
import Tokenizers

private struct ModelSpec: Codable {
    let model: String
    let version: Int
    let embeddingDim: Int
    let imageSize: Int
    let contextLength: Int
}

private final class TextTokenizer: @unchecked Sendable {
    private let tokenizer: Tokenizer
    private let contextLength: Int

    init(folder: URL, contextLength: Int) async throws {
        tokenizer = try await AutoTokenizer.from(modelFolder: folder)
        self.contextLength = contextLength
    }

    func tokenize(_ text: String) -> [Int32] {
        var ids = tokenizer.encode(text: text).map(Int32.init)
        if ids.count > contextLength {
            ids = Array(ids.prefix(contextLength))
        }
        ids += Array(repeating: 0, count: contextLength - ids.count)
        return ids
    }
}

private final class Encoder: @unchecked Sendable {
    private let spec: ModelSpec
    private let imageEncoder: MLModel
    private let textEncoder: MLModel
    private let tokenizer: TextTokenizer

    init(modelDirectory: URL) async throws {
        let decoder = JSONDecoder()
        spec = try decoder.decode(
            ModelSpec.self,
            from: Data(contentsOf: modelDirectory.appendingPathComponent("spec.json"))
        )
        let configuration = MLModelConfiguration()
        configuration.computeUnits = .all
        imageEncoder = try MLModel(
            contentsOf: modelDirectory.appendingPathComponent("ImageEncoder.mlmodelc"),
            configuration: configuration
        )
        textEncoder = try MLModel(
            contentsOf: modelDirectory.appendingPathComponent("TextEncoder.mlmodelc"),
            configuration: configuration
        )
        tokenizer = try await TextTokenizer(
            folder: modelDirectory.appendingPathComponent("tokenizer", isDirectory: true),
            contextLength: spec.contextLength
        )
    }

    func encode(text: String) throws -> [Float] {
        let tokens = tokenizer.tokenize(text)
        let array = try MLMultiArray(
            shape: [1, NSNumber(value: spec.contextLength)],
            dataType: .int32
        )
        for (index, token) in tokens.enumerated() {
            array[index] = NSNumber(value: token)
        }
        let input = try MLDictionaryFeatureProvider(
            dictionary: ["tokens": MLFeatureValue(multiArray: array)]
        )
        return try Self.embedding(
            from: textEncoder.prediction(from: input),
            dimensions: spec.embeddingDim
        )
    }

    func encode(imageData: Data) throws -> [Float] {
        guard let source = CGImageSourceCreateWithData(imageData as CFData, nil),
              let image = CGImageSourceCreateImageAtIndex(source, 0, nil)
        else {
            throw RuntimeError("image bytes could not be decoded")
        }
        let buffer = try Self.pixelBuffer(from: image, size: spec.imageSize)
        let input = try MLDictionaryFeatureProvider(
            dictionary: ["image": MLFeatureValue(pixelBuffer: buffer)]
        )
        return try Self.embedding(
            from: imageEncoder.prediction(from: input),
            dimensions: spec.embeddingDim
        )
    }

    private static func embedding(
        from output: MLFeatureProvider,
        dimensions: Int
    ) throws -> [Float] {
        guard let array = output.featureValue(for: "embedding")?.multiArrayValue,
              array.count == dimensions
        else {
            throw RuntimeError("model returned an invalid embedding")
        }
        if array.dataType == .float32 {
            return array.withUnsafeBufferPointer(ofType: Float.self) { Array($0) }
        }
        return (0..<dimensions).map { array[$0].floatValue }
    }

    private static func pixelBuffer(from image: CGImage, size: Int) throws -> CVPixelBuffer {
        var optionalBuffer: CVPixelBuffer?
        let attributes = [
            kCVPixelBufferCGImageCompatibilityKey: true,
            kCVPixelBufferCGBitmapContextCompatibilityKey: true,
        ] as CFDictionary
        let result = CVPixelBufferCreate(
            kCFAllocatorDefault,
            size,
            size,
            kCVPixelFormatType_32BGRA,
            attributes,
            &optionalBuffer
        )
        guard result == kCVReturnSuccess, let buffer = optionalBuffer else {
            throw RuntimeError("pixel buffer allocation failed")
        }

        CVPixelBufferLockBaseAddress(buffer, [])
        defer { CVPixelBufferUnlockBaseAddress(buffer, []) }
        guard let context = CGContext(
            data: CVPixelBufferGetBaseAddress(buffer),
            width: size,
            height: size,
            bitsPerComponent: 8,
            bytesPerRow: CVPixelBufferGetBytesPerRow(buffer),
            space: CGColorSpace(name: CGColorSpace.sRGB) ?? CGColorSpaceCreateDeviceRGB(),
            bitmapInfo: CGImageAlphaInfo.premultipliedFirst.rawValue
                | CGBitmapInfo.byteOrder32Little.rawValue
        ) else {
            throw RuntimeError("pixel buffer drawing context failed")
        }
        context.interpolationQuality = .high
        context.setFillColor(CGColor(gray: 0, alpha: 1))
        context.fill(CGRect(x: 0, y: 0, width: size, height: size))
        context.draw(image, in: CGRect(x: 0, y: 0, width: size, height: size))
        return buffer
    }
}

private struct Request: Decodable {
    let id: UInt64
    let type: String
    let text: String?
    let imageBase64: String?
}

private struct Response: Encodable {
    let id: UInt64
    let embedding: [Float]?
    let error: String?
}

private struct RuntimeError: LocalizedError {
    let message: String
    init(_ message: String) { self.message = message }
    var errorDescription: String? { message }
}

private func compiledModelURL(
    modelDirectory: URL,
    name: String
) async throws -> URL {
    let compiled = modelDirectory.appendingPathComponent("\(name).mlmodelc", isDirectory: true)
    if FileManager.default.fileExists(atPath: compiled.path) {
        return compiled
    }
    let source = modelDirectory.appendingPathComponent("\(name).mlpackage", isDirectory: true)
    guard FileManager.default.fileExists(atPath: source.path) else {
        throw RuntimeError("missing \(name).mlpackage")
    }
    let temporary = try await MLModel.compileModel(at: source)
    try? FileManager.default.removeItem(at: compiled)
    try FileManager.default.moveItem(at: temporary, to: compiled)
    return compiled
}

private func prepare(modelDirectory: URL) async throws {
    _ = try await compiledModelURL(modelDirectory: modelDirectory, name: "ImageEncoder")
    _ = try await compiledModelURL(modelDirectory: modelDirectory, name: "TextEncoder")
    let tokenizer = modelDirectory.appendingPathComponent("tokenizer/tokenizer.json")
    guard FileManager.default.fileExists(atPath: tokenizer.path) else {
        throw RuntimeError("missing tokenizer/tokenizer.json")
    }
}

private func serve(modelDirectory: URL) async throws {
    try await prepare(modelDirectory: modelDirectory)
    let encoder = try await Encoder(modelDirectory: modelDirectory)
    let decoder = JSONDecoder()
    let output = JSONEncoder()
    while let line = readLine(strippingNewline: true) {
        let response: Response
        do {
            let request = try decoder.decode(Request.self, from: Data(line.utf8))
            do {
                let embedding: [Float]
                switch request.type {
                case "text":
                    guard let text = request.text, !text.trimmingCharacters(in: .whitespacesAndNewlines).isEmpty else {
                        throw RuntimeError("text request is blank")
                    }
                    embedding = try encoder.encode(text: text)
                case "image":
                    guard let encoded = request.imageBase64,
                          let bytes = Data(base64Encoded: encoded)
                    else {
                        throw RuntimeError("image request has invalid base64")
                    }
                    embedding = try encoder.encode(imageData: bytes)
                default:
                    throw RuntimeError("unknown request type \(request.type)")
                }
                response = Response(id: request.id, embedding: embedding, error: nil)
            } catch {
                response = Response(id: request.id, embedding: nil, error: error.localizedDescription)
            }
        } catch {
            response = Response(id: 0, embedding: nil, error: error.localizedDescription)
        }
        let bytes = try output.encode(response)
        print(String(decoding: bytes, as: UTF8.self))
        fflush(stdout)
    }
}

@main
private enum Main {
    static func main() async {
        do {
            let arguments = Array(CommandLine.arguments.dropFirst())
            guard arguments.count == 2, arguments[1].hasPrefix("--model-dir=") else {
                throw RuntimeError("usage: video-creater-semantic-encoder <prepare|serve> --model-dir=<path>")
            }
            let directory = URL(fileURLWithPath: String(arguments[1].dropFirst("--model-dir=".count)))
            switch arguments[0] {
            case "prepare": try await prepare(modelDirectory: directory)
            case "serve": try await serve(modelDirectory: directory)
            default: throw RuntimeError("unknown command \(arguments[0])")
            }
        } catch {
            FileHandle.standardError.write(Data("semantic encoder failed: \(error.localizedDescription)\n".utf8))
            exit(1)
        }
    }
}
