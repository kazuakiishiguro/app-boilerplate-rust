/*****************************************************************************
 *   Ledger App Boilerplate Rust.
 *   (c) 2023 Ledger SAS.
 *
 *  Licensed under the Apache License, Version 2.0 (the "License");
 *  you may not use this file except in compliance with the License.
 *  You may obtain a copy of the License at
 *
 *      http://www.apache.org/licenses/LICENSE-2.0
 *
 *  Unless required by applicable law or agreed to in writing, software
 *  distributed under the License is distributed on an "AS IS" BASIS,
 *  WITHOUT WARRANTIES OR CONDITIONS OF ANY KIND, either express or implied.
 *  See the License for the specific language governing permissions and
 *  limitations under the License.
 *****************************************************************************/

#![no_std]
#![no_main]

extern crate alloc;

use include_gif::include_gif;
use ledger_device_sdk::io::{ApduHeader, Comm, Event, Reply, StatusWords};
use ledger_device_sdk::ui::{
    bagls::CHECKMARK_ICON,
    bitmaps::Glyph,
    gadgets::clear_screen,
    layout::{Draw, Layout, Location, StringPlace},
    SCREEN_HEIGHT, SCREEN_WIDTH,
};
use ledger_device_sdk::uxapp::{BOLOS_UX_CONTINUE, BOLOS_UX_IGNORE};
use ledger_secure_sdk_sys::seph as sys_seph;
use ledger_secure_sdk_sys::*;

ledger_device_sdk::set_panic!(ledger_device_sdk::exiting_panic);

const ICON_PIXEL_SIZE: usize = 14;
const ICON_MIDDLE: usize = (SCREEN_HEIGHT - ICON_PIXEL_SIZE) / 2;
const ICON_CENTERED: i16 = ((SCREEN_WIDTH - ICON_PIXEL_SIZE) / 2) as i16;

#[repr(u16)]
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum AppSW {
    Deny = 0x6985,
    WrongP1P2 = 0x6A86,
    InsNotSupported = 0x6D00,
    ClaNotSupported = 0x6E00,
    TxDisplayFail = 0xB001,
    AddrDisplayFail = 0xB002,
    TxWrongLength = 0xB004,
    TxParsingFail = 0xB005,
    TxHashFail = 0xB006,
    TxSignFail = 0xB008,
    KeyDeriveFail = 0xB009,
    VersionParsingFail = 0xB00A,
    WrongApduLength = StatusWords::BadLen as u16,
    Ok = 0x9000,
}

impl From<AppSW> for Reply {
    fn from(sw: AppSW) -> Reply {
        Reply(sw as u16)
    }
}

/// Possible input commands received through APDUs.
pub enum Instruction {
    GetVersion,
}

impl TryFrom<ApduHeader> for Instruction {
    type Error = AppSW;
    fn try_from(value: ApduHeader) -> Result<Self, Self::Error> {
        match (value.ins, value.p1, value.p2) {
            (3, 0, 0) => Ok(Instruction::GetVersion),
            (_, _, _) => Err(AppSW::InsNotSupported),
        }
    }
}

#[no_mangle]
extern "C" fn sample_main() {
    let mut comm = Comm::new().set_expected_cla(0xe0);

    display_top();

    loop {
        let event = comm.next_event::<ApduHeader>();
        match event {
            Event::Command(ins) => match handle_apdu(&mut comm, &ins.try_into().unwrap()) {
                Ok(()) => {
                    comm.reply_ok();
                }
                Err(sw) => {
                    comm.reply(sw);
                }
            },
            _ => {}
        }
    }
}

fn handle_apdu(comm: &mut Comm, ins: &Instruction) -> Result<(), AppSW> {
    match ins {
        Instruction::GetVersion => handler_get_version(comm),
    }
}

fn display_top() {
    clear_screen();
    const APP_ICON: Glyph = Glyph::from_include(include_gif!("crab.gif"));
    APP_ICON.draw(0, 0)
}

fn handler_get_version(comm: &mut Comm) -> Result<(), AppSW> {
    clear_screen();
    let checkmark = CHECKMARK_ICON.set_x(ICON_CENTERED).set_y(8);
    checkmark.display();
    "Message Received".place(Location::Custom(ICON_MIDDLE + 8), Layout::Centered, true);
    wait_ticker::<Instruction>(comm);
    display_top();
    Ok(())
}

fn wait_ticker<T>(comm: &mut Comm)
where
    T: TryFrom<ApduHeader>,
    Reply: From<<T as TryFrom<ApduHeader>>::Error>,
{
    let mut elapsed_time = 0;
    let max_elapsed_time = 2000;

    while elapsed_time < max_elapsed_time {
        let event = UxEvent::block_and_get_event::<T>(comm);

        match event {
            Some(Event::Ticker) => {
                elapsed_time += 100;
            }
            _ => {}
        }
        if elapsed_time > max_elapsed_time {
            break;
        }
    }
}

fn os_ux_rs(params: &bolos_ux_params_t) {
    unsafe { os_ux(params as *const bolos_ux_params_t as *mut bolos_ux_params_t) };
}

#[repr(u8)]
pub enum UxEvent {
    Event = BOLOS_UX_EVENT,
    Keyboard = BOLOS_UX_KEYBOARD,
    WakeUp = BOLOS_UX_WAKE_UP,
    ValidatePIN = BOLOS_UX_VALIDATE_PIN,
    LastID = BOLOS_UX_LAST_ID,
}

impl UxEvent {
    pub fn request(&self) -> u32 {
        let mut params = bolos_ux_params_t::default();
        params.ux_id = match self {
            Self::Event => Self::Event as u8,
            Self::Keyboard => Self::Keyboard as u8,
            Self::WakeUp => Self::WakeUp as u8,
            Self::ValidatePIN => {
                // Perform pre-wake up
                params.ux_id = Self::WakeUp as u8;
                os_ux_rs(&params);

                Self::ValidatePIN as u8
            }
            Self::LastID => Self::LastID as u8,
        };

        os_ux_rs(&params);

        match self {
            Self::ValidatePIN => Self::block(),
            _ => unsafe { os_sched_last_status(TASK_BOLOS_UX as u32) as u32 },
        }
    }

    pub fn block() -> u32 {
        let mut ret = unsafe { os_sched_last_status(TASK_BOLOS_UX as u32) } as u32;
        while ret == BOLOS_UX_IGNORE || ret == BOLOS_UX_CONTINUE {
            if unsafe { os_sched_is_running(TASK_SUBTASKS_START as u32) }
                != BOLOS_TRUE.try_into().unwrap()
            {
                let mut spi_buffer = [0u8; 256];
                sys_seph::send_general_status();
                sys_seph::seph_recv(&mut spi_buffer, 0);
                UxEvent::Event.request();
            } else {
                unsafe { os_sched_yield(BOLOS_UX_OK as u8) };
            }
            ret = unsafe { os_sched_last_status(TASK_BOLOS_UX as u32) } as u32;
        }
        ret
    }

    pub fn block_and_get_event<T>(comm: &mut Comm) -> Option<Event<T>>
    where
        T: TryFrom<ApduHeader>,
        Reply: From<<T as TryFrom<ApduHeader>>::Error>,
    {
        let mut spi_buffer = [0u8; 256];
        seph::send_general_status();
        seph::seph_recv(&mut spi_buffer, 0);
        let event = comm.decode_event(&mut spi_buffer);

        UxEvent::Event.request();

        if let Option::Some(Event::Ticker) = event {
            return event;
        }
        event
    }
}
