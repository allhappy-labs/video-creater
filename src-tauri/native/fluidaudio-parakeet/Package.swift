// swift-tools-version: 5.9

import PackageDescription

let package = Package(
    name: "VideoCreaterFluidAudioTranscribe",
    platforms: [
        .macOS(.v14)
    ],
    products: [
        .library(
            name: "VideoCreaterFluidAudioTranscribeCore",
            targets: ["VideoCreaterFluidAudioTranscribeCore"]
        ),
        .executable(
            name: "video-creater-fluidaudio-transcribe",
            targets: ["VideoCreaterFluidAudioTranscribe"]
        )
    ],
    dependencies: [
        .package(
            url: "https://github.com/FluidInference/FluidAudio.git",
            revision: "3c6e79f1d74411cae1f3daf50260dd19a585dc2d"
        )
    ],
    targets: [
        .target(
            name: "VideoCreaterFluidAudioTranscribeCore",
            dependencies: [
                .product(name: "FluidAudio", package: "FluidAudio")
            ]
        ),
        .executableTarget(
            name: "VideoCreaterFluidAudioTranscribe",
            dependencies: ["VideoCreaterFluidAudioTranscribeCore"]
        ),
        .testTarget(
            name: "VideoCreaterFluidAudioTranscribeTests",
            dependencies: [
                "VideoCreaterFluidAudioTranscribeCore",
                .product(name: "FluidAudio", package: "FluidAudio"),
            ]
        )
    ]
)
