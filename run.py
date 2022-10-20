import argparse
import subprocess
import os
import pathlib


root_dir = pathlib.Path(__file__).parent
parser = argparse.ArgumentParser()
parser.add_argument('--bios', action='store_true', help='Build this project with a legacy BIOS bootloader')
parser.add_argument('--debug', action='store_true', help='Run in qemu debug mode?')
parser.add_argument('--build-only', action='store_true', help='Build project without running it')


def build_with_bios(base_cargo_args) -> int:
    BUILD_DIR = f'{root_dir}/target/x86_64-bios-target/debug'
    cargo = [*base_cargo_args, '--features', 'bios']
    cargo_env = dict(os.environ, RUSTFLAGS=f'-C link-args={root_dir}/linker.ld')
    objcopy_strip_debug = ['objcopy', '--only-keep-debug', f'{BUILD_DIR}/bootloader', f'{BUILD_DIR}/bmb_sym']
    objcopy_output_binary = ['objcopy', '-O', 'binary', f'{BUILD_DIR}/bootloader', f'{BUILD_DIR}/bmb_bin']
    cargo_exit_code = subprocess.run(cargo, env=cargo_env).returncode
    if cargo_exit_code != 0:
        return cargo_exit_code
    objcopy_exit_code = subprocess.run(objcopy_strip_debug).returncode
    if objcopy_exit_code != 0:
        return objcopy_exit_code
    subprocess.run(objcopy_output_binary).returncode
                

def run_with_bios(base_qemu_args) -> None:
    BUILD_DIR = f'{root_dir}/target/x86_64-bios-target/debug'
    qemu  = base_qemu_args + ['-drive', f'file={BUILD_DIR}/bmb_bin,format=raw']
    subprocess.run(qemu)


def build_with_uefi(base_cargo_args) -> int:
    return subprocess.run(base_cargo_args).returncode

def run_with_uefi(base_qemu_args) -> None:
    BUILD_DIR = f'{root_dir}/target/x86_64-unknown-uefi/debug'
    OVMF_ROOT = '/usr/share/edk2/ovmf'
    subprocess.run([
        *base_qemu_args,
        '-s',
        '-net', 'none',
        #'-soundhw', '',
        #'-debugcon', 'file:debug.log',
        #'-global', 'isa-debugcon.iobase=0x402',
        '-drive', f'if=pflash,format=raw,unit=0,file=OVMF_CODE.fd,readonly=on',
        '-drive', f'if=pflash,unit=1,format=raw,file=OVMF_VARS.fd',
        '-drive', f'format=raw,file=fat:rw:{BUILD_DIR}',
        '-cpu', 'qemu64'
    ])


if __name__ == '__main__':
    args = parser.parse_args()
    target = f'{root_dir}/x86_64-bios-target.json' if args.bios else 'x86_64-unknown-uefi'
    base_cargo_args = [
        'cargo', 'b', '-p', 'bootloader', '--target', target,
        '-Zbuild-std=core,compiler_builtins', '-Zbuild-std-features=compiler-builtins-mem',
    ]
    base_qemu_args = ['qemu-system-x86_64', 
        '-device', 'ich9-intel-hda,debug=4', '-device', 'hda-micro', '-device', 'hda-micro']
    if args.debug:
        base_qemu_args += ['-S', '-s']
    if args.bios:
        if build_with_bios(base_cargo_args) == 0:
            if not args.build_only:
                run_with_bios(base_qemu_args)
    else:
        if build_with_uefi(base_cargo_args) == 0:
            if not args.build_only:
                run_with_uefi(base_qemu_args)
    

