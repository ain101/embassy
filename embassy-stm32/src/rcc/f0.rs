use crate::pac::rcc::vals::{Hpre, Hsebyp, Pllmul, Pllsrc, Ppre, Sw, Usbsw};
use crate::pac::{FLASH, RCC};
use crate::time::Hertz;

use super::{set_freqs, Clocks};

const HSI: u32 = 8_000_000;

/// Configuration of the clocks
///
/// hse takes precedence over hsi48 if both are enabled
#[non_exhaustive]
#[derive(Default)]
pub struct Config {
    pub hse: Option<Hertz>,
    pub bypass_hse: bool,
    pub usb_pll: bool,

    #[cfg(rcc_f0)]
    pub hsi48: bool,

    pub sys_ck: Option<Hertz>,
    pub hclk: Option<Hertz>,
    pub pclk: Option<Hertz>,
}

pub(crate) unsafe fn init(config: Config) {
    let sysclk = config.sys_ck.map(|v| v.0).unwrap_or(HSI);

    let (src_clk, use_hsi48) = config.hse.map(|v| (v.0, false)).unwrap_or_else(|| {
        #[cfg(rcc_f0)]
        if config.hsi48 {
            return (48_000_000, true);
        }
        (HSI, false)
    });

    let (pllmul_bits, real_sysclk) = if sysclk == src_clk {
        (None, sysclk)
    } else {
        let prediv = if config.hse.is_some() { 1 } else { 2 };
        let pllmul = (2 * prediv * sysclk + src_clk) / src_clk / 2;
        let pllmul = pllmul.max(2).min(16);

        let pllmul_bits = pllmul as u8 - 2;
        let real_sysclk = pllmul * src_clk / prediv;
        (Some(pllmul_bits), real_sysclk)
    };

    let hpre_bits = config
        .hclk
        .map(|hclk| match real_sysclk / hclk.0 {
            0 => unreachable!(),
            1 => 0b0111,
            2 => 0b1000,
            3..=5 => 0b1001,
            6..=11 => 0b1010,
            12..=39 => 0b1011,
            40..=95 => 0b1100,
            96..=191 => 0b1101,
            192..=383 => 0b1110,
            _ => 0b1111,
        })
        .unwrap_or(0b0111);
    let hclk = real_sysclk / (1 << (hpre_bits - 0b0111));

    let ppre_bits = config
        .pclk
        .map(|pclk| match hclk / pclk.0 {
            0 => unreachable!(),
            1 => 0b011,
            2 => 0b100,
            3..=5 => 0b101,
            6..=11 => 0b110,
            _ => 0b111,
        })
        .unwrap_or(0b011);

    let ppre: u8 = 1 << (ppre_bits - 0b011);
    let pclk = hclk / u32::from(ppre);

    let timer_mul = if ppre == 1 { 1 } else { 2 };

    FLASH.acr().write(|w| {
        let latency = if real_sysclk <= 24_000_000 {
            0
        } else if real_sysclk <= 48_000_000 {
            1
        } else {
            2
        };
        w.latency().0 = latency;
    });

    match (config.hse.is_some(), use_hsi48) {
        (true, _) => {
            RCC.cr().modify(|w| {
                w.set_csson(true);
                w.set_hseon(true);

                if config.bypass_hse {
                    w.set_hsebyp(Hsebyp::BYPASSED);
                }
            });
            while !RCC.cr().read().hserdy() {}

            if pllmul_bits.is_some() {
                RCC.cfgr().modify(|w| w.set_pllsrc(Pllsrc::HSE_DIV_PREDIV))
            }
        }
        (false, true) => {
            // use_hsi48 will always be false for rcc_f0x0
            #[cfg(rcc_f0)]
            RCC.cr2().modify(|w| w.set_hsi48on(true));
            #[cfg(rcc_f0)]
            while !RCC.cr2().read().hsi48rdy() {}

            #[cfg(rcc_f0)]
            if pllmul_bits.is_some() {
                RCC.cfgr()
                    .modify(|w| w.set_pllsrc(Pllsrc::HSI48_DIV_PREDIV))
            }
        }
        _ => {
            RCC.cr().modify(|w| w.set_hsion(true));
            while !RCC.cr().read().hsirdy() {}

            if pllmul_bits.is_some() {
                RCC.cfgr().modify(|w| w.set_pllsrc(Pllsrc::HSI_DIV2))
            }
        }
    }

    if config.usb_pll {
        RCC.cfgr3().modify(|w| w.set_usbsw(Usbsw::PLLCLK));
    }
    // TODO: Option to use CRS (Clock Recovery)

    if let Some(pllmul_bits) = pllmul_bits {
        RCC.cfgr().modify(|w| w.set_pllmul(Pllmul(pllmul_bits)));

        RCC.cr().modify(|w| w.set_pllon(true));
        while !RCC.cr().read().pllrdy() {}

        RCC.cfgr().modify(|w| {
            w.set_ppre(Ppre(ppre_bits));
            w.set_hpre(Hpre(hpre_bits));
            w.set_sw(Sw::PLL)
        });
    } else {
        RCC.cfgr().modify(|w| {
            w.set_ppre(Ppre(ppre_bits));
            w.set_hpre(Hpre(hpre_bits));

            if config.hse.is_some() {
                w.set_sw(Sw::HSE);
            } else if use_hsi48 {
                #[cfg(rcc_f0)]
                w.set_sw(Sw::HSI48);
            } else {
                w.set_sw(Sw::HSI)
            }
        })
    }

    set_freqs(Clocks {
        sys: Hertz(real_sysclk),
        apb1: Hertz(pclk),
        apb2: Hertz(pclk),
        apb1_tim: Hertz(pclk * timer_mul),
        apb2_tim: Hertz(pclk * timer_mul),
        ahb: Hertz(hclk),
    });
}
