# Overview
This is the blasterball game for x86_64 from scratch, no game engine, no OS.
The only external libraries used are the Rust core library (because without this
library, Rust wouldn't be Rust) and the compiler_builtins library, because the core
library depends on it.

More detailed info on how the game works is at https://d-sonuga.netlify.app/game-from-scratch/

![Blasterball Gameplay](https://github.com/d-sonuga/bare-metal-blasterball/blob/assets/blasterball-gameplay.gif)

I had a lot of fun building this project, but it has a lot of loose ends and I won't
be able to get back to those loose ends because of time. The physics is really wonky, ACPI
isn't fully supported and the game itself has only background music but no sound effects.

# Running
I developed this on a Linux Fedora 38 system, so if you're on Windows, ..., I don't know what to
tell you.

## Requirements
* To run the code, you need a python3 interpreter installed, and I think that should come
with most Linux systems.

* Install qemu and OVMF (https://wiki.osdev.org/OVMF) for UEFI emulation

    `sudo dnf install qemu edk2-ovmf`

    or
    
    `sudo apt install qemu edk2-ovmf`

* Copy the OVMF_CODE.fd and OVMF_VARS.fd from the OVMF root directory (`/usr/share/edk2/ovmf` on my system)
to the root of this project

* Because of some features I used and decisions I made, you need the 
`nightly-2022-08-26` toolchain installed.

    To install this toolchain: `rustup install nightly-2022-08-26`

## Running in the emulator

* Run the Python script

    `python3 run.py`

* When the shell loads in the emulator loads up, type in

    `fs0:bootloader.efi`
    
    and hit enter

## Running on your machine
* Build the project

    `python run.py --build-only --release`

* Copy the `bootloader.efi` in target/x86_64-unknown-uefi/debug file to a flash drive
* Shutdown your computer
* Power on your computer again, and open the boot menu
* Choose boot from efi file
* Select the `bootloader.efi` in your flash drive root
