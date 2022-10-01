use core::{mem, slice};
use artist::println;

pub struct WavFile {
    header: &'static WavHeader,
    data: SampleDataChunk
}

/// The header of a wav file as described in <http://soundfile.sapp.org/doc/WaveFormat/>
///
/// The wav file format is an RIFF file type which is a generic file container format
/// for storing data in tagged chunks.
/// The file consists of chunks which have the format 4 bit ascii id, followed by
/// a 32 bit integer which is the length of the chunk, 
/// All ascii values are big-endian and all integer values are little endian
#[repr(C)]
struct WavHeader {
    file_chunk_header: RIFFChunkHeader,
    /// Always the ascii "WAVE"
    format: [u8; 4],

    /// Beginning of a RIFF chunk which gives some info about the data in the file
    ///
    /// The id is always the ascii "fmt ", and the size is 16 for PCM
    fmt_chunk_header: RIFFChunkHeader,
    /// Audio format
    ///
    /// A value of 1 indicates PCM. Values other than 1
    /// indicate a form of compression
    type_format: u16,
    /// The number of channels
    ///
    /// 1 for mono and 2 for stereo
    num_of_channels: u16,
    /// The sample frequency
    sample_rate: u32,
    /// `sample_rate` * `num_of_channels` * `bits_per_sample` / 8
    byte_rate: u32,
    /// Number of bytes for one sample including all channels,
    /// that is, `num_of_channels` * `bits_per_sample` / 8
    block_align: u16,
    /// Number of bits in a single sample
    bits_per_sample: u16
}

/// The beginning of every chunk in an RIFF file type
#[repr(C, packed)]
struct RIFFChunkHeader {
    /// A 4 byte ascii field that identifies the chunk
    id: [u8; 4],
    /// Size of the data in the chunk
    size: u32
}

/// The RIFF chunk in a wav file that contains sample data
#[repr(C)]
struct SampleDataChunk {
    header: RIFFChunkHeader,
    data: &'static [u8]
}

impl WavFile {
    pub unsafe fn from(file: &'static [u8]) -> Result<WavFile, &'static str> {
        let header = &*(file.as_ptr() as *const WavHeader);
        validate_header(header)?;
        let data_ptr = find_data_chunk(file);
        if data_ptr.is_none() {
            return Err("Couldn't find the data chunk");
        }
        let data_ptr = data_ptr.unwrap();
        const RIFF_HEADER_SIZE: isize = mem::size_of::<RIFFChunkHeader>() as isize;
        let data_chunk_header = data_ptr.cast::<RIFFChunkHeader>().read();
        let sample_data_ptr = data_ptr.offset(RIFF_HEADER_SIZE);
        let sample_data = core::slice::from_raw_parts(data_ptr, data_chunk_header.size as usize);
        Ok(Self {
            header,
            data: SampleDataChunk {
                header: data_chunk_header,
                data: sample_data
            }
        })
    }

    pub fn sample_rate(&self) -> u32 {
        self.header.sample_rate
    }

    pub fn num_of_channels(&self) -> u16 {
        self.header.num_of_channels
    }

    pub fn bits_per_sample(&self) -> u16 {
        self.header.bits_per_sample
    }
}

unsafe fn find_data_chunk(file: &[u8]) -> Option<*const u8> {
    let header = &*(file.as_ptr().cast::<RIFFChunkHeader>());
    //let bytes = header.as_ptr().cast::<u8>().offset(HEADER_SIZE);
    let bytes = core::slice::from_raw_parts(file.as_ptr(), header.size as usize);
    for chunk in bytes.array_windows::<4>() {
        // Chunk id of the data chunk is "data"
        if chunk == b"data" {
            return Some(chunk.as_ptr());
        }
    }
    None
}

/// Checks if the WavHeader in a raw byte stream is valid
fn validate_header(header: &WavHeader) -> Result<(), &'static str> {
    if &header.file_chunk_header.id != b"RIFF" {
        return Err("Unexpected file chunk id");
    }
    if &header.format != b"WAVE" {
        return Err("Unexpected format");
    }
    if &header.fmt_chunk_header.id != b"fmt " {
        return Err("Unexpected fmt_chunk_header id");
    }
    Ok(())
}
