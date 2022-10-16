import argparse
import subprocess
import os
import pathlib


root_dir = pathlib.Path(__file__).parent
parser = argparse.ArgumentParser()
parser.add_argument('--bios', action='store_true', help='Build this project with a legacy BIOS bootloader')
parser.add_argument('--debug', action='store_true', help='Run in qemu debug mode?')


def run_with_bios(base_cargo_args, base_qemu_args):
    BUILD_DIR = f'{root_dir}/target/x86_64-bios-target/debug'
    cargo = [*base_cargo_args, '--features', 'bios']
    cargo_env = dict(os.environ, RUSTFLAGS=f'-C link-args={root_dir}/linker.ld')
    objcopy_strip_debug = ['objcopy', '--only-keep-debug', f'{BUILD_DIR}/bootloader', f'{BUILD_DIR}/bmb_sym']
    objcopy_output_binary = ['objcopy', '-O', 'binary', f'{BUILD_DIR}/bootloader', f'{BUILD_DIR}/bmb_bin']
    qemu  = base_qemu_args + ['-drive', f'file={BUILD_DIR}/bmb_bin,format=raw']
    if subprocess.run(cargo, env=cargo_env).returncode == 0:
        if subprocess.run(objcopy_strip_debug).returncode == 0:
            if subprocess.run(objcopy_output_binary).returncode == 0:
                subprocess.run(qemu)


def run_with_uefi(base_cargo_args, base_qemu_args):
    root_dir = pathlib.Path(__file__).parent
    BUILD_DIR = f'{root_dir}/target/x86_64-unknown-uefi/debug'
    OVMF_ROOT = '/usr/share/edk2/ovmf'
    if subprocess.run(base_cargo_args).returncode == 0:
        subprocess.run([
            #'sudo',
            *base_qemu_args,
            '-s',
            #'-serial', 'tcp::666,server',
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
        run_with_bios(base_cargo_args, base_qemu_args)
    else:
        run_with_uefi(base_cargo_args, base_qemu_args)

