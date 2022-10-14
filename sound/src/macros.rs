pub use lazy_static::lazy_static;

#[macro_export]
macro_rules! sound {
    ($name:ident, $raw_name:ident => $location:expr, size => $size:expr) => {
        #[link_section = ".sound"]
        static $raw_name: [u8; $size] = *include_bytes!($location);
        $crate::macros::lazy_static! {
            #[link_section = ".sound"]
            static ref $name: Sound = {
                #[repr(C, align(128))]
                struct SB([Sample; $size / 2]);
                impl core::ops::Deref for SB {
                    type Target = [Sample];
                    fn deref(&self) -> &Self::Target {
                        self.0.as_slice()
                    }
                }
                impl core::ops::DerefMut for SB {
                    fn deref_mut(&mut self) -> &mut Self::Target {
                        self.0.as_mut_slice()
                    }
                }
                #[link_section = ".sound"]
                static mut SAMPLE_BUFFER: SB = {
                    SB([Sample(0); $size / 2])
                };
                let music = WavFile::from(&$raw_name).unwrap();
                let sound = sound::Sound::new(music, unsafe { &mut SAMPLE_BUFFER });
                sound
            };
        }
    }
}