// swift-tools-version: 5.10

import PackageDescription

let package = Package(
    name: "VideoCreaterSemanticEncoder",
    platforms: [.macOS("15.0")],
    products: [
        .executable(
            name: "video-creater-semantic-encoder",
            targets: ["VideoCreaterSemanticEncoder"]
        )
    ],
    dependencies: [
        .package(
            url: "https://github.com/huggingface/swift-transformers",
            revision: "2fa33e1f5e7131a7fc64c28e6d161dcec0d24820"
        )
    ],
    targets: [
        .executableTarget(
            name: "VideoCreaterSemanticEncoder",
            dependencies: [
                .product(name: "Tokenizers", package: "swift-transformers")
            ]
        )
    ]
)
