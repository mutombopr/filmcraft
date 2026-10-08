//! VA-API decoding against our software decoders (Linux): every picture of the H.264 fixture
//! must be identical (bit-exact planes, colour, pixel aspect, pts and presentation order), also
//! after `reset` + reseek; damaged samples must give errors or fall back, never crash or hang.
//! Skips without ffmpeg (fixture generator) or without a VA-API driver that decodes H.264.
#![cfg(target_os = "linux")]

mod common;

use common::*;
use filmcraft_codecs::VideoDecoder;

/// The hardware counters are process-wide: tests that read them must not overlap.
static SERIAL: std::sync::Mutex<()> = std::sync::Mutex::new(());

/// The hardware decoder for a stream, or `None` (skip) when this machine has none for it.
fn hardware(s: &Stream) -> Option<Box<dyn VideoDecoder>> {
    filmcraft_codecs::hw::set_hardware_decoding(true);
    match filmcraft_platform::vaapi_factory(&s.entry) {
        Some(Ok(d)) => Some(d),
        Some(Err(e)) => panic!("factory error: {e}"),
        None => {
            eprintln!("SKIPPED: no VA-API hardware decoder for {}", s.entry.codec.name());
            None
        }
    }
}

fn software(s: &Stream) -> Box<dyn VideoDecoder> {
    filmcraft_codecs::software_video_decoder(&s.entry).unwrap()
}

#[test]
fn h264_is_bit_exact_with_the_software_decoder() {
    let _serial = SERIAL.lock().unwrap_or_else(std::sync::PoisonError::into_inner);
    let ff = filmcraft_testkit::require_ffmpeg!();
    let Some(path) = named(&ff, "h264_high.mp4") else { return };
    let s = read_stream(&path);
    let Some(mut hw) = hardware(&s) else { return };
    let mut sw = software(&s);
    let before = filmcraft_codecs::hw::hw_stats();
    let a = decode_all(hw.as_mut(), &s.samples);
    let b = decode_all(sw.as_mut(), &s.samples);
    assert!(a.len() >= 60, "{} pictures", a.len());
    assert_same("h264_high", &a, &b);
    let after = filmcraft_codecs::hw::hw_stats();
    // the pictures really came from the GPU (stats are global: compare this run only)
    assert_eq!(after.fallbacks - before.fallbacks, 0, "stayed in hardware: {before:?} -> {after:?}");
    assert_eq!(after.frames - before.frames, a.len() as u64, "every picture decoded in hardware: {before:?} -> {after:?}");
    // reset + reseek to each later sync sample, decode a stretch
    let syncs: Vec<usize> = (1..s.samples.len()).filter(|&i| s.sync[i]).collect();
    assert!(!syncs.is_empty(), "more than one GOP");
    for &k in syncs.iter().rev() {
        let end = (k + 17).min(s.samples.len());
        hw.reset();
        sw.reset();
        let a = decode_all(hw.as_mut(), &s.samples[k..end]);
        let b = decode_all(sw.as_mut(), &s.samples[k..end]);
        assert!(!a.is_empty(), "pictures after seeking to {k}");
        assert_same(&format!("h264_high from sample {k}"), &a, &b);
    }
    hw.reset();
    sw.reset();
    assert_same("h264_high after resets", &decode_all(hw.as_mut(), &s.samples), &decode_all(sw.as_mut(), &s.samples));
}

#[test]
fn damaged_samples_never_crash() {
    let _serial = SERIAL.lock().unwrap_or_else(std::sync::PoisonError::into_inner);
    let ff = filmcraft_testkit::require_ffmpeg!();
    let Some(path) = named(&ff, "h264_high.mp4") else { return };
    let s = read_stream(&path);
    let mut seed = 0x5eed_u64;
    for round in 0..4 {
        let Some(mut hw) = hardware(&s) else { return };
        for (i, (smp, pts)) in s.samples.iter().enumerate() {
            let mut d = smp.clone();
            if i > 0 && !d.is_empty() && round > 0 {
                for _ in 0..round * 3 {
                    let at = (xorshift(&mut seed) as usize) % d.len();
                    d[at] ^= (xorshift(&mut seed) & 0xff) as u8;
                }
                if round == 3 {
                    d.truncate(d.len() / 2);
                }
            }
            let _ = hw.decode(&d, *pts);
        }
        let _ = hw.flush();
    }
}
