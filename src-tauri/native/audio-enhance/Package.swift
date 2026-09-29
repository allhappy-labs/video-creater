// swift-tools-version: 5.10

import PackageDescription

let package = Package(
    name: "VideoCreaterAudioEnhance",
    platforms: [.macOS("15.0")],
    products: [
        .executable(
            name: "video-creater-audio-enhance",
            targets: ["VideoCreaterAudioEnhance"]
        )
    ],
    dependencies: [
        .package(
            url: "https://github.com/soniqo/speech-swift.git",
            revision: "7609977be837a6529bd04300c6b963e735300070"
        )
    ],
    targets: [
        .executableTarget(
            name: "VideoCreaterAudioEnhance",
            dependencies: [
                .product(name: "SpeechEnhancement", package: "speech-swift")
            ],
            path: "Sources"
        )
    ]
)
