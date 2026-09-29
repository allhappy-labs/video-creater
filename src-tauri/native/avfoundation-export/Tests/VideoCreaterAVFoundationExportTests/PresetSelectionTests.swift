import AVFoundation
import XCTest
@testable import VideoCreaterAVFoundationExport

final class PresetSelectionTests: XCTestCase {
    func testH264DraftUsesBoundedPresetWhileFinalUsesHighestQuality() {
        XCTAssertEqual(
            exportPresetName(profile: .h264, quality: .draft, width: 1920, height: 1080),
            AVAssetExportPreset1920x1080
        )
        XCTAssertEqual(
            exportPresetName(profile: .h264, quality: .final, width: 1920, height: 1080),
            AVAssetExportPresetHighestQuality
        )
    }

    func testHevcDraftPreservesHevcWithBoundedPreset() {
        XCTAssertEqual(
            exportPresetName(profile: .hevc, quality: .draft, width: 1920, height: 1080),
            AVAssetExportPresetHEVC1920x1080
        )
        XCTAssertEqual(
            exportPresetName(profile: .hevc, quality: .final, width: 1920, height: 1080),
            AVAssetExportPresetHEVCHighestQuality
        )
    }
}
