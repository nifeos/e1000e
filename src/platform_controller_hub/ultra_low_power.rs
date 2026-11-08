/// `FWSM.FW_VALID`: a Management Engine is present and running, so it owns the ULP exit.
pub(crate) const ICH_FWSM_FW_VALID: u32 = 0x0000_8000;
/// `FWSM.ULP_CFG_DONE`: set while the Management Engine's ULP configuration is still in force.
pub(crate) const FWSM_ULP_CFG_DONE: u32 = 0x0000_0400;
/// `H2ME.ULP`: the host's ULP indication.
pub(crate) const H2ME_ULP: u32 = 0x0000_0800;
/// `H2ME.ENFORCE_SETTINGS`: ask the Management Engine to apply the host's request.
pub(crate) const H2ME_ENFORCE_SETTINGS: u32 = 0x0000_1000;
/// How long to wait for the management engine, in milliseconds: FreeBSD's 250 polls of 10 ms.
pub const MANAGEMENT_ENGINE_WAIT_MILLISECONDS: u64 = 2500;

/// The `H2ME` value that asks the Management Engine to take the PHY out of ULP (the forced path).
pub const fn register_value_requesting_exit(host_to_management_engine: u32) -> u32 {
    (host_to_management_engine & !H2ME_ULP) | H2ME_ENFORCE_SETTINGS
}
/// The `H2ME` value once the Management Engine has answered.
pub const fn register_value_releasing_request(host_to_management_engine: u32) -> u32 {
    host_to_management_engine & !H2ME_ENFORCE_SETTINGS
}
/// Is the Management Engine's ULP configuration finished?
pub const fn is_configuration_done(firmware_semaphore: u32) -> bool {
    firmware_semaphore & FWSM_ULP_CFG_DONE == 0
}
/// Does this machine have a Management Engine to ask? When it does not, ULP exit is the PHY-register
/// sequence in [`super::sequence`].
pub(crate) const fn has_management_engine(firmware_semaphore: u32) -> bool {
    firmware_semaphore & ICH_FWSM_FW_VALID != 0
}
