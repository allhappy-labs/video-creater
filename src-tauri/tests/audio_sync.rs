use video_creater_lib::audio_sync::AudioSyncCorrelator;

#[test]
fn audio_sync_correlator_reports_one_hop_negative_lag_for_delayed_target() {
    let reference = vec![
        0.0, 0.0, 0.1, 0.2, 0.8, 1.0, 0.7, 0.2, 0.1, 0.0, 0.2, 0.6, 0.9, 0.6, 0.2, 0.0, 0.1, 0.3,
        0.7, 0.4, 0.1, 0.0, 0.0, 0.0,
    ];
    let target = vec![
        0.0, 0.0, 0.0, 0.1, 0.2, 0.8, 1.0, 0.7, 0.2, 0.1, 0.0, 0.2, 0.6, 0.9, 0.6, 0.2, 0.0, 0.1,
        0.3, 0.7, 0.4, 0.1, 0.0, 0.0,
    ];

    let result =
        AudioSyncCorrelator::correlate(&reference, &target, 4).expect("correlation result");

    assert_eq!(result.lag_hops, -1);
    assert_eq!(result.confidence, 1.0);
}

#[test]
fn audio_sync_correlator_rejects_empty_or_zero_lag_inputs() {
    assert!(AudioSyncCorrelator::correlate(&[], &[1.0], 4).is_none());
    assert!(AudioSyncCorrelator::correlate(&[1.0], &[], 4).is_none());
    assert!(AudioSyncCorrelator::correlate(&[0.0, 1.0], &[0.0, 1.0], 0).is_none());
}
