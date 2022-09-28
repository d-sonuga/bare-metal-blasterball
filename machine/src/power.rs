use core::slice;
use core::mem;
use crate::port::{Port, PortReadWrite};
use crate::memory::Addr;
use crate::acpi::{detect_rsdp, SDTTable, RSDP};
use sync::once::Once;

pub static FRAMEBUFFER: Once<Addr> = Once::new();

/// Shuts down the computer
///
/// If it's successful, the Ok(()) will never be returned
/// An error is returned whenever anything expected isn't found, or anything
/// goes wrong.
///
/// # References:
/// * https://wiki.osdev.org/Shutdown
/// * https://wiki.osdev.org/RSDP
/// * https://wiki.osdev.org/RSDT
/// * https://wiki.osdev.org/FADT
/// * https://exchangetuts.com/how-to-power-down-the-computer-from-a-freestanding-environment-1640049843947324
/// * https://wiki.osdev.org/AML
/// * https://wiki.osdev.org/DSDT
/// * https://forum.osdev.org/viewtopic.php?t=16990
pub unsafe fn shutdown() -> Result<(), ()> {
    let rsdp = detect_rsdp();
    if rsdp.is_none() {
        return Err(());
    }
    let rsdp = rsdp.unwrap();
    if rsdp == RSDP::None {
        //Couldn't find the RSDP
        return Err(())
    }
    if !rsdp.is_valid() {
        //RSDP table is not valid
        return Err(())
    }
    let rsdt = &*rsdp.rsdt_ptr();
    if !rsdt.is_valid() {
        // RSDT isn't valid
        return Err(())
    }
    let fadt = rsdt.find_fadt();
    if fadt.is_none() {
        // Didn't fin FADT
        return Err(())
    }
    let fadt = fadt.unwrap();
    if !fadt.is_valid() {
        // FADT isn't valid
        return Err(())
    }
    let dsdt = &*fadt.dsdt_ptr();
    if !dsdt.is_valid() {
        // DSDT isn't valid
        return Err(())
    }

    dsdt.figure_out_how_to_execute_the_pts_obj();

    // Shutting down requires PM1a_CTRL_BLOCK or PM1a_CTRL_BLOCK, SLP_TYPa or SLP_TYPb
    // And outw(PM1a_CTRL_BLOCK, SLP_TYPa | SLP_EN) or outw(PM1b_CTRL_BLOCK, SLP_TYPb | SLP_EN)
    // should be run to shut down
    // The SLP_TYPa as SLP_TYPb are in the DSDT (and it's AML encoded)
    // The PM1a_CTRL_BLOCK and PM1b_CTRL_BLOCK are in the FADT
    let slp_en = 1 << 13;
    let slp_typ_opt = dsdt.get_slp_typ();
    if slp_typ_opt.is_none() {
        // Didn't find the SLP_TYPa and SLP_TYPb
        return Err(());
    }
    let (slp_typa, slp_typb) = slp_typ_opt.unwrap();
    let mut port: Port<u16> = Port::new(fadt.pm1a_ctrl_block() as u16);
    port.write(slp_typa as u16 | slp_en);
    if fadt.pm1b_ctrl_block() != 0 {
        let mut port: Port<u16> = Port::new(fadt.pm1b_ctrl_block() as u16);
        port.write(slp_typb as u16 | slp_en);
    }
    Err(())
}
