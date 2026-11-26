//! Writing generated audio out as WAV, MP3 or Ogg Vorbis.
//!
//! The engine produces mono `f32` samples at the model's own sample rate. Both save paths use
//! [`AudioSink`], so a long render streams straight to disk instead of being collected in memory,
//! and [`wav_to_pcm`] decodes the WAV a frontend already holds so it can be re-encoded.

use anyhow::{Context, Result, bail};
use std::fs::File;
use std::io::{BufWriter, Seek, SeekFrom, Write};
use std::num::{NonZeroU32, NonZeroU8};
use std::path::Path;

/// Saved MP3 bitrate. 192 kbps mono is transparent enough for speech without inflating files.
const MP3_BITRATE: mp3lame_encoder::Bitrate = mp3lame_encoder::Bitrate::Kbps192;

/// The largest block handed to the Vorbis encoder: libvorbis is tuned for blocks of up to a few
/// thousand samples and degrades sharply above that, so larger chunks are sliced.
const OGG_BLOCK: usize = 4096;

/// Output format, chosen by the target file extension.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Codec {
    Wav,
    Mp3,
    Ogg,
}

impl Codec {
    /// The codec a file name asks for. Unknown extensions are an error rather than a silent
    /// fallback, so a typo cannot produce a `.mp3` that is really RIFF.
    pub fn for_path(path: &Path) -> Result<Self> {
        let ext = path
            .extension()
            .and_then(|e| e.to_str())
            .unwrap_or_default()
            .to_ascii_lowercase();
        Ok(match ext.as_str() {
            "wav" | "wave" => Codec::Wav,
            "mp3" => Codec::Mp3,
            "ogg" | "oga" => Codec::Ogg,
            "" => bail!("no file extension: use .wav, .mp3 or .ogg"),
            other => bail!("unsupported audio format .{other}: use .wav, .mp3 or .ogg"),
        })
    }

    pub fn name(self) -> &'static str {
        match self {
            Codec::Wav => "wav",
            Codec::Mp3 => "mp3",
            Codec::Ogg => "ogg",
        }
    }
}

/// A 44-byte mono 16-bit PCM WAV header describing `data_bytes` of samples.
fn wav_header(sample_rate: u32, data_bytes: u32) -> [u8; 44] {
    let mut header = [0u8; 44];
    header[0..4].copy_from_slice(b"RIFF");
    header[4..8].copy_from_slice(&(36 + data_bytes).to_le_bytes());
    header[8..12].copy_from_slice(b"WAVE");
    header[12..16].copy_from_slice(b"fmt ");
    header[16..20].copy_from_slice(&16u32.to_le_bytes());
    header[20..22].copy_from_slice(&1u16.to_le_bytes()); // PCM
    header[22..24].copy_from_slice(&1u16.to_le_bytes()); // mono
    header[24..28].copy_from_slice(&sample_rate.to_le_bytes());
    header[28..32].copy_from_slice(&(sample_rate * 2).to_le_bytes());
    header[32..34].copy_from_slice(&2u16.to_le_bytes());
    header[34..36].copy_from_slice(&16u16.to_le_bytes());
    header[36..40].copy_from_slice(b"data");
    header[40..44].copy_from_slice(&data_bytes.to_le_bytes());
    header
}

fn sample_bytes(pcm: &[f32]) -> Vec<u8> {
    let mut bytes = Vec::with_capacity(pcm.len() * 2);
    for &sample in pcm {
        let s = (sample.clamp(-1.0, 1.0) * 32767.0) as i16;
        bytes.extend_from_slice(&s.to_le_bytes());
    }
    bytes
}

/// Mono 16-bit little-endian samples, the format a resumed render keeps beside its output.
pub(crate) fn pcm_to_pcm16(pcm: &[f32]) -> Vec<u8> {
    sample_bytes(pcm)
}

/// The inverse of [`pcm_to_pcm16`]; a trailing odd byte is ignored.
pub(crate) fn pcm16_to_pcm(bytes: &[u8]) -> Vec<f32> {
    bytes
        .chunks_exact(2)
        .map(|pair| i16::from_le_bytes([pair[0], pair[1]]) as f32 / 32768.0)
        .collect()
}

/// A WAV being written as audio arrives. The RIFF sizes are patched in when it is finished.
pub(crate) struct WavWriter {
    file: BufWriter<File>,
    sample_rate: u32,
    samples: u64,
}

impl WavWriter {
    fn create(path: &Path, sample_rate: u32) -> Result<Self> {
        let file = File::create(path).with_context(|| format!("cannot create {}", path.display()))?;
        let mut file = BufWriter::new(file);
        file.write_all(&wav_header(sample_rate, 0))?;
        Ok(Self {
            file,
            sample_rate,
            samples: 0,
        })
    }

    fn write(&mut self, pcm: &[f32]) -> Result<()> {
        self.file.write_all(&sample_bytes(pcm))?;
        self.samples += pcm.len() as u64;
        Ok(())
    }

    fn finish(self) -> Result<u64> {
        let data_bytes = u32::try_from(self.samples * 2).unwrap_or(u32::MAX);
        let mut file = self.file.into_inner().map_err(|e| e.into_error())?;
        file.seek(SeekFrom::Start(0))?;
        file.write_all(&wav_header(self.sample_rate, data_bytes))?;
        file.flush()?;
        Ok(self.samples)
    }
}

/// LAME fed one block at a time, with its output appended to the file as it is produced.
pub(crate) struct Mp3Writer {
    encoder: mp3lame_encoder::Encoder,
    file: BufWriter<File>,
    /// Reused so a long render does not allocate a buffer per chunk.
    buffer: Vec<u8>,
    samples: u64,
}

impl Mp3Writer {
    fn create(path: &Path, sample_rate: u32) -> Result<Self> {
        let mut builder = mp3lame_encoder::Builder::new().context("LAME encoder unavailable")?;
        // LAME's error types are plain enums without an `Error` impl, so they are stringified.
        builder
            .set_num_channels(1)
            .map_err(|e| anyhow::anyhow!("LAME: mono unsupported ({e:?})"))?;
        builder
            .set_sample_rate(sample_rate)
            .map_err(|e| {
                anyhow::anyhow!("LAME: {sample_rate} Hz is not a supported MP3 rate ({e:?})")
            })?;
        builder
            .set_brate(MP3_BITRATE)
            .map_err(|e| anyhow::anyhow!("LAME: bitrate ({e:?})"))?;
        builder
            .set_quality(mp3lame_encoder::Quality::Best)
            .map_err(|e| anyhow::anyhow!("LAME: quality ({e:?})"))?;
        let encoder = builder
            .build()
            .map_err(|e| anyhow::anyhow!("LAME: encoder init failed ({e:?})"))?;
        let file = File::create(path).with_context(|| format!("cannot create {}", path.display()))?;
        Ok(Self {
            encoder,
            file: BufWriter::new(file),
            buffer: Vec::new(),
            samples: 0,
        })
    }

    fn write(&mut self, pcm: &[f32]) -> Result<()> {
        if pcm.is_empty() {
            return Ok(());
        }
        let needed = mp3lame_encoder::max_required_buffer_size(pcm.len()) + 8192;
        self.buffer.clear();
        self.buffer.reserve(needed);
        let written = self
            .encoder
            .encode(
                mp3lame_encoder::MonoPcm(pcm),
                self.buffer.spare_capacity_mut(),
            )
            .map_err(|e| anyhow::anyhow!("LAME: encode failed ({e:?})"))?;
        // The count comes from the encoder; grow the vector only when it is within the
        // capacity we handed over, so a wrong count can never read past the allocation.
        anyhow::ensure!(
            written <= self.buffer.capacity(),
            "LAME wrote {written} bytes into a {}-byte buffer",
            self.buffer.capacity()
        );
        // SAFETY: the first `written` bytes were initialised by the encoder and checked above.
        unsafe { self.buffer.set_len(written) };
        self.file.write_all(&self.buffer)?;
        self.samples += pcm.len() as u64;
        Ok(())
    }

    fn finish(mut self) -> Result<u64> {
        let mut tail: Vec<u8> = Vec::with_capacity(16384);
        let written = self
            .encoder
            .flush::<mp3lame_encoder::FlushNoGap>(tail.spare_capacity_mut())
            .map_err(|e| anyhow::anyhow!("LAME: flush failed ({e:?})"))?;
        anyhow::ensure!(
            written <= tail.capacity(),
            "LAME wrote {written} bytes into a {}-byte buffer",
            tail.capacity()
        );
        // SAFETY: as above - the encoder reports the bytes it wrote, checked against capacity.
        unsafe { tail.set_len(written) };
        self.file.write_all(&tail)?;
        self.file.flush()?;
        Ok(self.samples)
    }
}

pub(crate) struct OggWriter {
    encoder: vorbis_rs::VorbisEncoder<BufWriter<File>>,
    samples: u64,
}

impl OggWriter {
    fn create(path: &Path, sample_rate: u32) -> Result<Self> {
        let rate = NonZeroU32::new(sample_rate).context("invalid sample rate")?;
        let channels = NonZeroU8::new(1).expect("mono is non-zero");
        let file = File::create(path).with_context(|| format!("cannot create {}", path.display()))?;
        let mut builder = vorbis_rs::VorbisEncoderBuilder::new(rate, channels, BufWriter::new(file))
            .context("Ogg Vorbis encoder unavailable")?;
        let encoder = builder.build().context("Ogg Vorbis: init failed")?;
        Ok(Self {
            encoder,
            samples: 0,
        })
    }

    fn write(&mut self, pcm: &[f32]) -> Result<()> {
        for block in pcm.chunks(OGG_BLOCK) {
            self.encoder
                .encode_audio_block(&[block])
                .context("Ogg Vorbis: encode failed")?;
        }
        self.samples += pcm.len() as u64;
        Ok(())
    }

    fn finish(self) -> Result<u64> {
        let mut file = self.encoder.finish().context("Ogg Vorbis: finish failed")?;
        file.flush()?;
        Ok(self.samples)
    }
}

/// Where a render is written: chosen by the codec, streamed block by block.
pub enum AudioSink {
    Wav(WavWriter),
    Mp3(Mp3Writer),
    Ogg(OggWriter),
}

impl AudioSink {
    /// Creates the parent directory when the user picked a path inside a new folder.
    pub fn create(codec: Codec, path: &Path, sample_rate: u32) -> Result<Self> {
        if let Some(parent) = path.parent() {
            if !parent.as_os_str().is_empty() {
                let _ = std::fs::create_dir_all(parent);
            }
        }
        Ok(match codec {
            Codec::Wav => AudioSink::Wav(WavWriter::create(path, sample_rate)?),
            Codec::Mp3 => AudioSink::Mp3(Mp3Writer::create(path, sample_rate)?),
            Codec::Ogg => AudioSink::Ogg(OggWriter::create(path, sample_rate)?),
        })
    }

    pub fn write(&mut self, pcm: &[f32]) -> Result<()> {
        match self {
            AudioSink::Wav(w) => w.write(pcm),
            AudioSink::Mp3(w) => w.write(pcm),
            AudioSink::Ogg(w) => w.write(pcm),
        }
    }

    /// Closes the file and returns the number of samples written.
    pub fn finish(self) -> Result<u64> {
        match self {
            AudioSink::Wav(w) => w.finish(),
            AudioSink::Mp3(w) => w.finish(),
            AudioSink::Ogg(w) => w.finish(),
        }
    }
}

/// Encodes a finished block of samples in memory, for audio the frontend already holds.
pub fn encode_in_memory(codec: Codec, pcm: &[f32], sample_rate: u32) -> Result<Vec<u8>> {
    match codec {
        Codec::Wav => Ok(pcm_to_wav(pcm, sample_rate)),
        Codec::Mp3 | Codec::Ogg => {
            // Write to a temporary file through the same streaming sink, so a single code path
            // owns the codec setup, then hand the bytes back.
            let temp = tempfile::Builder::new()
                .suffix(&format!(".{}", codec.name()))
                .tempfile()
                .context("cannot create a temporary file")?;
            {
                let mut sink = AudioSink::create(codec, temp.path(), sample_rate)?;
                sink.write(pcm)?;
                sink.finish()?;
            }
            std::fs::read(temp.path()).context("cannot read back the encoded audio")
        }
    }
}

/// A mono 16-bit PCM WAV of `pcm`.
pub fn pcm_to_wav(pcm: &[f32], sample_rate: u32) -> Vec<u8> {
    let data = sample_bytes(pcm);
    let mut wav = Vec::with_capacity(44 + data.len());
    wav.extend_from_slice(&wav_header(
        sample_rate,
        u32::try_from(data.len()).unwrap_or(u32::MAX),
    ));
    wav.extend_from_slice(&data);
    wav
}

/// Decodes 16-bit PCM or 32-bit float WAV bytes to mono samples and the sample rate.
///
/// This only has to cope with what the app itself produces (mono 16-bit) plus the float WAV a
/// browser preview may hand back, so it walks the RIFF chunks and rejects anything else rather
/// than guessing.
pub fn wav_to_pcm(bytes: &[u8]) -> Result<(Vec<f32>, u32)> {
    if bytes.len() < 44 || &bytes[0..4] != b"RIFF" || &bytes[8..12] != b"WAVE" {
        bail!("not a RIFF/WAVE file");
    }
    let mut format = 1u16;
    let mut channels = 1u16;
    let mut bits = 16u16;
    let mut sample_rate = 0u32;
    let mut data: Option<&[u8]> = None;

    let mut offset = 12;
    while offset + 8 <= bytes.len() {
        let id = &bytes[offset..offset + 4];
        let size = u32::from_le_bytes(bytes[offset + 4..offset + 8].try_into().unwrap()) as usize;
        let body_start = offset + 8;
        let body_end = body_start.saturating_add(size).min(bytes.len());
        if id == b"fmt " && body_end - body_start >= 16 {
            let body = &bytes[body_start..body_end];
            format = u16::from_le_bytes([body[0], body[1]]);
            channels = u16::from_le_bytes([body[2], body[3]]);
            sample_rate = u32::from_le_bytes([body[4], body[5], body[6], body[7]]);
            bits = u16::from_le_bytes([body[14], body[15]]);
        } else if id == b"data" {
            data = Some(&bytes[body_start..body_end]);
        }
        offset = body_start + size + (size % 2);
    }

    let data = data.context("WAV has no data chunk")?;
    if channels == 0 || sample_rate == 0 {
        bail!("WAV has no usable format chunk");
    }
    let frame_bytes = (bits as usize / 8) * channels as usize;
    if frame_bytes == 0 {
        bail!("unsupported WAV bit depth: {bits}");
    }
    let mut pcm = Vec::with_capacity(data.len() / frame_bytes);
    for frame in data.chunks_exact(frame_bytes) {
        let mut sum = 0.0f32;
        for channel in 0..channels as usize {
            let at = channel * (bits as usize / 8);
            let value = match (format, bits) {
                (1, 16) => {
                    i16::from_le_bytes([frame[at], frame[at + 1]]) as f32 / 32768.0
                }
                (3, 32) => f32::from_le_bytes([
                    frame[at],
                    frame[at + 1],
                    frame[at + 2],
                    frame[at + 3],
                ]),
                _ => bail!("unsupported WAV encoding: format {format}, {bits} bit"),
            };
            sum += value;
        }
        pcm.push(sum / channels as f32);
    }
    Ok((pcm, sample_rate))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn tone(samples: usize) -> Vec<f32> {
        (0..samples)
            .map(|i| (i as f32 * 0.05).sin() * 0.4)
            .collect()
    }

    #[test]
    fn codec_follows_the_extension() {
        assert_eq!(Codec::for_path(Path::new("a.WAV")).unwrap(), Codec::Wav);
        assert_eq!(Codec::for_path(Path::new("a.mp3")).unwrap(), Codec::Mp3);
        assert_eq!(Codec::for_path(Path::new("a.ogg")).unwrap(), Codec::Ogg);
        assert!(Codec::for_path(Path::new("a.flac")).is_err());
        assert!(Codec::for_path(Path::new("noext")).is_err());
    }

    #[test]
    fn wav_round_trips_through_the_decoder() {
        let pcm = tone(480);
        let wav = pcm_to_wav(&pcm, 24000);
        let (decoded, rate) = wav_to_pcm(&wav).unwrap();
        assert_eq!(rate, 24000);
        assert_eq!(decoded.len(), pcm.len());
        for (a, b) in pcm.iter().zip(decoded.iter()) {
            assert!((a - b).abs() < 2.0 / 32768.0);
        }
    }

    #[test]
    fn mp3_output_is_an_mpeg_stream() {
        let bytes = encode_in_memory(Codec::Mp3, &tone(24000), 24000).unwrap();
        assert!(bytes.len() > 1000, "mp3 output too small: {}", bytes.len());
        assert_eq!(bytes[0], 0xFF, "no MPEG frame sync at the start");
        assert_eq!(bytes[1] & 0xE0, 0xE0);
    }

    #[test]
    fn ogg_output_is_an_ogg_stream() {
        let bytes = encode_in_memory(Codec::Ogg, &tone(24000), 24000).unwrap();
        assert!(bytes.len() > 1000, "ogg output too small: {}", bytes.len());
        assert_eq!(&bytes[0..4], b"OggS");
    }

    #[test]
    fn pcm16_round_trips() {
        let pcm = tone(777);
        let back = pcm16_to_pcm(&pcm_to_pcm16(&pcm));
        assert_eq!(back.len(), pcm.len());
        for (a, b) in pcm.iter().zip(back.iter()) {
            assert!((a - b).abs() < 2.0 / 32768.0);
        }
    }

    #[test]
    fn wav_sink_patches_its_header() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("stream.wav");
        let mut sink = AudioSink::create(Codec::Wav, &path, 24000).unwrap();
        sink.write(&tone(100)).unwrap();
        sink.write(&tone(200)).unwrap();
        assert_eq!(sink.finish().unwrap(), 300);
        let (decoded, rate) = wav_to_pcm(&std::fs::read(&path).unwrap()).unwrap();
        assert_eq!(rate, 24000);
        assert_eq!(decoded.len(), 300);
    }
}
