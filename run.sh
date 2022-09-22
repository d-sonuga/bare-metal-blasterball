ROOT_DIR="$(pwd)"
TARGET_DIR=target/x86_64-blasterball/debug
# Have to invoke cargo in the packages directories for the
# specific config files to take effect
cd blasterball
if [ 0 -eq 0 ] #cargo b -p blasterball
then

cd ..
# Have to change from current directory to the target directory
# where all the outputs are
if cd "$TARGET_DIR"
then

# The blasterball package is compiled directly to binary
# Turn it into an elf file to be linked with the bootloader, which is
# compiled into an object format, and define some symbols used in the bootloader
#objcopy -I binary -O elf64-x86-64 --binary-architecture=i386:x86-64 --debugging --rename-section .data=.app --redefine-sym _binary_blasterball_start=_app_start_addr --redefine-sym _binary_blasterball_size=_app_size --redefine-sym _binary_blasterball_end=_app_end_addr blasterball app_bin.o

# Turn the app into a static library to be linked with the bootloader
#ar crs deps/libapp_bin_ar.a app_bin.o

# Build the bootloader, giving the app archive as a static library for linking
cd "$ROOT_DIR/bootloader"
if cargo b -p bootloader
then

# Get the linker script
#cp "$ROOT_DIR/linker.ld" .
cd "../$TARGET_DIR"
#objcopy -I elf64-x86-64 -O binary --binary-architecture=i386:x86-64 bootloader bmb_bin

# Link the bootloader with the app
#ld -o bb -T linker.ld deps/*.o app_bin.o

# For debugging
objcopy --only-keep-debug bootloader bmb_sym

objcopy -O binary bootloader bmb_bin
 
qemu-system-x86_64 -device intel-hda -device hda-duplex -drive file=bmb_bin,format=raw


else
echo "Failed to build the bootloader"
fi

else
echo "Failed to build the app"
fi

else
echo "You have to run from the root directory of the project"
fi
