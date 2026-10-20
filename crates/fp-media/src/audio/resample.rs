//! Converts decoded audio frames to interleaved f32 at 48 kHz.

use crate::decode::Frame;
use crate::{Error, Result, check};
use fp_ffmpeg_sys as ff;

pub struct Resampler {
    swr: *mut ff::SwrContext,
    key: (i32, i32, i32, u64),
    out_layout: ff::AVChannelLayout,
    pub out_channels: u32,
}

// SAFETY: used by the audio thread only.
unsafe impl Send for Resampler {}

impl Resampler {
    /// `keep_layout`: keep the source channel layout (ambisonics) instead of
    /// down-mixing to stereo.
    pub fn new(keep_layout_channels: Option<u32>) -> Resampler {
        // SAFETY: zeroed layout is the documented "unset" state.
        let mut out_layout: ff::AVChannelLayout = unsafe { std::mem::zeroed() };
        let ch = keep_layout_channels.unwrap_or(2);
        // SAFETY: out_layout is an unset layout owned here.
        unsafe { ff::av_channel_layout_default(&mut out_layout, ch as i32) };
        Resampler {
            swr: std::ptr::null_mut(),
            key: (0, 0, 0, 0),
            out_layout,
            out_channels: ch,
        }
    }

    /// Converts one frame, appending samples to `out`.
    pub fn convert(&mut self, frame: &Frame, out: &mut Vec<f32>) -> Result<()> {
        if frame.as_ptr().is_null() {
            return Ok(());
        }
        // SAFETY: frame is a decoded audio frame; swr is (re)created to match it.
        unsafe {
            let f = &*frame.as_ptr();
            if f.nb_samples <= 0 || f.sample_rate <= 0 || f.ch_layout.nb_channels <= 0 {
                return Ok(());
            }
            if self.out_channels == f.ch_layout.nb_channels as u32
                && f.ch_layout.order == ff::AV_CHANNEL_ORDER_AMBISONIC
                && self.out_layout.order != ff::AV_CHANNEL_ORDER_AMBISONIC
            {
                ff::av_channel_layout_uninit(&mut self.out_layout);
                ff::av_channel_layout_copy(&mut self.out_layout, &f.ch_layout);
                ff::swr_free(&mut self.swr);
            }
            let key = (
                f.format,
                f.sample_rate,
                f.ch_layout.nb_channels,
                f.ch_layout.u.mask,
            );
            if self.swr.is_null() || key != self.key {
                ff::swr_free(&mut self.swr);
                let mut swr = std::ptr::null_mut();
                check(
                    ff::swr_alloc_set_opts2(
                        &mut swr,
                        &self.out_layout,
                        ff::AV_SAMPLE_FMT_FLT,
                        super::RATE as i32,
                        &f.ch_layout,
                        f.format,
                        f.sample_rate,
                        0,
                        std::ptr::null_mut(),
                    ),
                    "audio resampler",
                )?;
                check(ff::swr_init(swr), "audio resampler init")?;
                self.swr = swr;
                self.key = key;
            }
            let max_out = ff::swr_get_out_samples(self.swr, f.nb_samples).max(0) as usize + 32;
            let ch = self.out_channels as usize;
            let start = out.len();
            out.resize(start + max_out * ch, 0.0);
            let dst = out[start..].as_mut_ptr() as *mut u8;
            let n = ff::swr_convert(
                self.swr,
                &dst,
                max_out as i32,
                f.extended_data as *mut *const u8,
                f.nb_samples,
            );
            if n < 0 {
                out.truncate(start);
                return Err(Error::Ffmpeg {
                    context: "resample",
                    code: n,
                    message: ff::err_to_string(n),
                });
            }
            out.truncate(start + n as usize * ch);
        }
        Ok(())
    }

    /// Appends the samples it still holds once the input has ended.
    pub fn flush(&mut self, out: &mut Vec<f32>) -> Result<()> {
        if self.swr.is_null() {
            return Ok(());
        }
        // SAFETY: swr is initialised; null input asks it for what it holds.
        unsafe {
            let max_out = ff::swr_get_out_samples(self.swr, 0).max(0) as usize + 32;
            let ch = self.out_channels as usize;
            let start = out.len();
            out.resize(start + max_out * ch, 0.0);
            let dst = out[start..].as_mut_ptr() as *mut u8;
            let n = ff::swr_convert(self.swr, &dst, max_out as i32, std::ptr::null(), 0);
            if n < 0 {
                out.truncate(start);
                return Err(Error::Ffmpeg {
                    context: "resample",
                    code: n,
                    message: ff::err_to_string(n),
                });
            }
            out.truncate(start + n as usize * ch);
        }
        Ok(())
    }

    pub fn reset(&mut self) {
        // SAFETY: frees our context; recreated lazily.
        unsafe { ff::swr_free(&mut self.swr) };
    }
}

impl Drop for Resampler {
    fn drop(&mut self) {
        // SAFETY: frees our own state.
        unsafe {
            ff::swr_free(&mut self.swr);
            ff::av_channel_layout_uninit(&mut self.out_layout);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn nothing_is_held_back_at_the_end() {
        // 0.1 s of silence at 44.1 kHz; converting to 48 kHz keeps a few
        // samples in the filter until flushed.
        let frame = Frame::new();
        // SAFETY: a fresh frame, given a layout and its own buffer.
        unsafe {
            let f = frame.as_ptr();
            (*f).format = ff::AV_SAMPLE_FMT_FLT;
            (*f).sample_rate = 44_100;
            (*f).nb_samples = 4_410;
            ff::av_channel_layout_default(&mut (*f).ch_layout, 2);
            assert!(ff::av_frame_get_buffer(f, 0) >= 0);
            std::ptr::write_bytes((*f).data[0], 0, 4_410 * 2 * 4);
        }
        let mut r = Resampler::new(None);
        let mut out = Vec::new();
        r.convert(&frame, &mut out).unwrap();
        let converted = out.len();
        r.flush(&mut out).unwrap();
        assert!(out.len() > converted, "nothing was held back");
        let frames = out.len() / 2;
        assert!(
            (4_790..=4_810).contains(&frames),
            "{frames} frames for 0.1 s"
        );
    }
}
