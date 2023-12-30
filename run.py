import argparse
import subprocess
import os
import pathlib


root_dir = pathlib.Path(__file__).parent
parser = argparse.ArgumentParser()
parser.add_argument('--bios', action='store_true', help='Build this project with a legacy BIOS bootloader')
parser.add_argument('--debug', action='store_true', help='Run in qemu debug mode?')
parser.add_argument('--build-only', action='store_true', help='Build project without running it')
parser.add_argument('--release', action='store_true', help='Build the project for release')


def build_with_bios(base_cargo_args, release=False) -> int:
    sub_dir = 'release' if release else 'debug'
    BUILD_DIR = f'{root_dir}/target/x86_64-bios-target/{sub_dir}'
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
    return subprocess.run(objcopy_output_binary).returncode
                

def run_with_bios(base_qemu_args, release=False) -> None:
    sub_dir = 'release' if release else 'debug'
    BUILD_DIR = f'{root_dir}/target/x86_64-bios-target/{sub_dir}'
    qemu  = base_qemu_args + ['-drive', f'file={BUILD_DIR}/bmb_bin,format=raw']
    subprocess.run(qemu)


def build_with_uefi(base_cargo_args) -> int:
    return subprocess.run(base_cargo_args).returncode

def run_with_uefi(base_qemu_args, release=False) -> None:
    sub_dir = 'release' if release else 'debug'
    BUILD_DIR = f'{root_dir}/target/x86_64-unknown-uefi/{sub_dir}'
    OVMF_ROOT = '/usr/share/edk2/ovmf'
    subprocess.run([
        *base_qemu_args,
        '-s',
        '-accel','kvm',
        '-drive', f'if=pflash,format=raw,unit=0,file=OVMF_CODE.fd,readonly=on',
        '-drive', f'if=pflash,unit=1,format=raw,file=OVMF_VARS.fd',
        '-drive', f'format=raw,file=fat:rw:{BUILD_DIR}',
        '-cpu', 'qemu64'
    ])


if __name__ == '__main__':
    args = parser.parse_args()
    target = f'{root_dir}/x86_64-bios-target.json' if args.bios else 'x86_64-unknown-uefi'
    base_cargo_args = [
        'cargo', '+nightly-2022-08-26', 'b', '-p', 'bootloader', '--target', target,
        '-Zbuild-std=core,compiler_builtins', '-Zbuild-std-features=compiler-builtins-mem',
    ]
    base_qemu_args = ['qemu-system-x86_64', 
        '-device', 'ich9-intel-hda,debug=4', '-device', 'hda-micro', '-device', 'hda-micro']
    if args.release:
        base_cargo_args += ['--release']
    if args.debug:
        base_qemu_args += ['-S', '-s']
    if args.bios:
        if build_with_bios(base_cargo_args, args.release) == 0:
            if not args.build_only:
                run_with_bios(base_qemu_args, args.release)
    else:
        if build_with_uefi(base_cargo_args) == 0:
            if not args.build_only:
                run_with_uefi(base_qemu_args, args.release)
    

