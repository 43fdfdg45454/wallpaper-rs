use super::*;

#[test]
fn the_report_counts_the_frames_of_the_last_thirty_seconds_and_says_how_long_each_part_took() {
    record(true);
    let stats = wprs_render::SceneStats { passes: 8, draws: 12, gpu_us: 2_500, ..Default::default() };
    for i in 0..=2_400_i64 {
        staged((1.0, 2.0), (Tier::T2, Quality::Q2, Why::Full), ((1080, 2400), (1080, 2400)), (60, 60), stats);
        presented(3.0, "fifo", 1);
        keep(i * 1_000_000_000 / 60, (0.5, 7.0), 3);
    }
    let text = report();
    record(false);
    assert!(text.contains("1801 presented in the last 30.0 s, 60.0 per second"), "{text}");
    assert!(text.contains("T2 Q2, full") && text.contains("fifo with latency 1"), "{text}");
    assert!(text.contains("draw cpu     mean 7.00") && text.contains("3.0 a frame"), "{text}");
    assert!(text.contains("encode     mean 2.00") && text.contains("8.0 passes, 12.0 draws"), "{text}");
    assert_eq!(report(), "performance recording is off");
}
