#![no_std]

use core::cell::RefCell;
use core::convert::Infallible;
use core::fmt;
use critical_section::Mutex;
use embassy_time_driver::{AlarmHandle, Driver};
use embedded_hal::delay::DelayNs;
use embedded_hal::digital::{ErrorType as DigitalErrorType, InputPin, OutputPin};
use embedded_hal::i2c::{Error as I2cError, ErrorKind as I2cErrorKind, ErrorType as I2cErrorType, I2c, NoAcknowledgeSource, Operation};
use embedded_hal::spi::{Error as SpiError, ErrorKind as SpiErrorKind, ErrorType as SpiErrorType, SpiBus};
use embedded_io::{ErrorType as IoErrorType, Write};
use thejas32_pac::{Gpio0, I2c0, Spi0, Uart0};

pub struct Serial0 { uart: Uart0 }

impl Serial0 {
    pub fn new(uart: Uart0) -> Self { Self { uart } }
    pub fn write_byte(&mut self, byte: u8) {
        while (self.uart.lsr().read().bits() & 0x20) == 0 {}
        self.uart.dr().write(|w| unsafe { w.bits(byte as u32) });
    }
}

impl IoErrorType for Serial0 { type Error = Infallible; }

impl Write for Serial0 {
    fn write(&mut self, buf: &[u8]) -> Result<usize, Self::Error> {
        for &b in buf { self.write_byte(b); }
        Ok(buf.len())
    }
    fn flush(&mut self) -> Result<(), Self::Error> {
        while (self.uart.lsr().read().bits() & 0x40) == 0 {}
        Ok(())
    }
}

impl fmt::Write for Serial0 {
    fn write_str(&mut self, s: &str) -> fmt::Result {
        for b in s.bytes() {
            if b == b'\n' { self.write_byte(b'\r'); }
            self.write_byte(b);
        }
        Ok(())
    }
}

pub struct Pin<const N: u8> { _private: () }

impl<const N: u8> Pin<N> {
    pub fn into_output(self) -> Self {
        let gpio = unsafe { &*Gpio0::ptr() };
        let dir = gpio.dir().read().bits();
        gpio.dir().write(|w| unsafe { w.bits(dir | (1 << N)) });
        self
    }
    pub fn into_input(self) -> Self {
        let gpio = unsafe { &*Gpio0::ptr() };
        let dir = gpio.dir().read().bits();
        gpio.dir().write(|w| unsafe { w.bits(dir & !(1 << N)) });
        self
    }
}

impl<const N: u8> DigitalErrorType for Pin<N> { type Error = Infallible; }

impl<const N: u8> OutputPin for Pin<N> {
    fn set_high(&mut self) -> Result<(), Self::Error> {
        let gpio = unsafe { &*Gpio0::ptr() };
        let val = gpio.data_all().read().bits();
        gpio.data_all().write(|w| unsafe { w.bits(val | (1 << N)) });
        Ok(())
    }
    fn set_low(&mut self) -> Result<(), Self::Error> {
        let gpio = unsafe { &*Gpio0::ptr() };
        let val = gpio.data_all().read().bits();
        gpio.data_all().write(|w| unsafe { w.bits(val & !(1 << N)) });
        Ok(())
    }
}

impl<const N: u8> InputPin for Pin<N> {
    fn is_high(&mut self) -> Result<bool, Self::Error> {
        let gpio = unsafe { &*Gpio0::ptr() };
        Ok((gpio.data_all().read().bits() & (1 << N)) != 0)
    }
    fn is_low(&mut self) -> Result<bool, Self::Error> { Ok(!self.is_high()?) }
}

pub struct Gpio0Pins { pub p0: Pin<0>, pub p1: Pin<1>, pub p2: Pin<2>, pub p13: Pin<13> }

impl Gpio0Pins {
    pub fn split(_gpio: Gpio0) -> Self {
        Self { p0: Pin { _private: () }, p1: Pin { _private: () }, p2: Pin { _private: () }, p13: Pin { _private: () } }
    }
}

#[derive(Debug, Copy, Clone, Eq, PartialEq)]
pub enum SpiBusError { Overrun, ModeFault }

impl SpiError for SpiBusError {
    fn kind(&self) -> SpiErrorKind {
        match self { SpiBusError::Overrun => SpiErrorKind::Overrun, SpiBusError::ModeFault => SpiErrorKind::ModeFault }
    }
}

pub struct SpiMaster0 { spi: Spi0 }

impl SpiMaster0 {
    pub fn new(spi: Spi0, prescaler: u32) -> Self {
        spi.baud().write(|w| unsafe { w.bits(prescaler) });
        spi.cr().write(|w| unsafe { w.bits(0x01) });
        Self { spi }
    }
    fn transfer_byte(&mut self, byte: u8) -> Result<u8, SpiBusError> {
        while (self.spi.sr().read().bits() & 0x04) != 0 {}
        self.spi.txdr().write(|w| unsafe { w.bits(byte as u32) });
        while (self.spi.sr().read().bits() & 0x02) == 0 {}
        Ok((self.spi.rxdr().read().bits() & 0xFF) as u8)
    }
}

impl SpiErrorType for SpiMaster0 { type Error = SpiBusError; }

impl SpiBus<u8> for SpiMaster0 {
    fn read(&mut self, words: &mut [u8]) -> Result<(), Self::Error> {
        for word in words.iter_mut() { *word = self.transfer_byte(0xFF)?; }
        Ok(())
    }
    fn write(&mut self, words: &[u8]) -> Result<(), Self::Error> {
        for &word in words { let _ = self.transfer_byte(word)?; }
        Ok(())
    }
    fn transfer(&mut self, read: &mut [u8], write: &[u8]) -> Result<(), Self::Error> {
        let len = read.len().max(write.len());
        for i in 0..len {
            let rx = self.transfer_byte(*write.get(i).unwrap_or(&0xFF))?;
            if let Some(slot) = read.get_mut(i) { *slot = rx; }
        }
        Ok(())
    }
    fn transfer_in_place(&mut self, words: &mut [u8]) -> Result<(), Self::Error> {
        for word in words.iter_mut() { *word = self.transfer_byte(*word)?; }
        Ok(())
    }
    fn flush(&mut self) -> Result<(), Self::Error> {
        while (self.spi.sr().read().bits() & 0x04) != 0 {}
        Ok(())
    }
}

#[derive(Debug, Copy, Clone, Eq, PartialEq)]
pub enum I2cBusError { AddressNack, DataNack, BusError }

impl I2cError for I2cBusError {
    fn kind(&self) -> I2cErrorKind {
        match self {
            I2cBusError::AddressNack => I2cErrorKind::NoAcknowledge(NoAcknowledgeSource::Address),
            I2cBusError::DataNack => I2cErrorKind::NoAcknowledge(NoAcknowledgeSource::Data),
            I2cBusError::BusError => I2cErrorKind::Bus,
        }
    }
}

pub struct I2cMaster0 { i2c: I2c0 }

impl I2cMaster0 {
    pub fn new(i2c: I2c0, prescaler: u32) -> Self {
        i2c.prer().write(|w| unsafe { w.bits(prescaler) });
        Self { i2c }
    }
    fn wait_done(&self, is_addr: bool) -> Result<(), I2cBusError> {
        loop {
            let sr = self.i2c.sr().read().bits();
            if (sr & 0x02) != 0 { return Err(if is_addr { I2cBusError::AddressNack } else { I2cBusError::DataNack }); }
            if (sr & 0x04) != 0 { return Ok(()); }
        }
    }
}

impl I2cErrorType for I2cMaster0 { type Error = I2cBusError; }

impl I2c for I2cMaster0 {
    fn transaction(&mut self, address: u8, operations: &mut [Operation<'_>]) -> Result<(), Self::Error> {
        let total_ops = operations.len();
        for (op_idx, op) in operations.iter_mut().enumerate() {
            let is_last_op = op_idx + 1 == total_ops;
            self.i2c.sar().write(|w| unsafe { w.bits((address & 0x7F) as u32) });
            match op {
                Operation::Write(bytes) => {
                    if bytes.is_empty() {
                        self.i2c.cr().write(|w| unsafe { w.bits(0x01 | if is_last_op { 0x02 } else { 0 }) });
                        self.wait_done(true)?;
                    } else {
                        for (i, &b) in bytes.iter().enumerate() {
                            let mut cmd = 0x08 | if i == 0 { 0x01 } else { 0 };
                            if is_last_op && i + 1 == bytes.len() { cmd |= 0x02; }
                            self.i2c.dr().write(|w| unsafe { w.bits(b as u32) });
                            self.i2c.cr().write(|w| unsafe { w.bits(cmd) });
                            self.wait_done(i == 0)?;
                        }
                    }
                }
                Operation::Read(buf) => {
                    let len = buf.len();
                    for (i, slot) in buf.iter_mut().enumerate() {
                        let mut cmd = 0x04 | if i == 0 { 0x01 } else { 0 };
                        if i + 1 < len { cmd |= 0x10; } else if is_last_op { cmd |= 0x02; }
                        self.i2c.cr().write(|w| unsafe { w.bits(cmd) });
                        self.wait_done(i == 0)?;
                        *slot = (self.i2c.dr().read().bits() & 0xFF) as u8;
                    }
                }
            }
        }
        Ok(())
    }
}

pub struct CycleDelay { sysclk_hz: u32 }

impl CycleDelay { pub fn new(sysclk_hz: u32) -> Self { Self { sysclk_hz } } }

impl DelayNs for CycleDelay {
    fn delay_ns(&mut self, ns: u32) {
        let cycles = ((self.sysclk_hz as u64) * (ns as u64)) / 1_000_000_000;
        let start = riscv::register::mcycle::read64();
        while riscv::register::mcycle::read64().wrapping_sub(start) < cycles {}
    }
}

const CLINT_MTIME_LO: *const u32 = 0x0200_BFF8 as *const u32;
const CLINT_MTIME_HI: *const u32 = 0x0200_BFFC as *const u32;
const CLINT_MTIMECMP_LO: *mut u32 = 0x0200_4000 as *mut u32;
const CLINT_MTIMECMP_HI: *mut u32 = 0x0200_4004 as *mut u32;
const TICKS_PER_US: u64 = 1;

#[derive(Copy, Clone)]
struct AlarmState { timestamp: u64, callback: Option<(fn(*mut ()), *mut ())> }
unsafe impl Send for AlarmState {}

struct ClintDriver {
    alarms: Mutex<RefCell<[AlarmState; 4]>>,
    allocated: Mutex<RefCell<u8>>,
}

impl ClintDriver {
    const fn new() -> Self {
        const EMPTY: AlarmState = AlarmState { timestamp: u64::MAX, callback: None };
        Self { alarms: Mutex::new(RefCell::new([EMPTY; 4])), allocated: Mutex::new(RefCell::new(0)) }
    }
    fn read_mtime() -> u64 {
        unsafe {
            loop {
                let hi1 = core::ptr::read_volatile(CLINT_MTIME_HI);
                let lo = core::ptr::read_volatile(CLINT_MTIME_LO);
                let hi2 = core::ptr::read_volatile(CLINT_MTIME_HI);
                if hi1 == hi2 { return ((hi1 as u64) << 32) | (lo as u64); }
            }
        }
    }
    fn write_mtimecmp(val: u64) {
        unsafe {
            core::ptr::write_volatile(CLINT_MTIMECMP_LO, u32::MAX);
            core::ptr::write_volatile(CLINT_MTIMECMP_HI, (val >> 32) as u32);
            core::ptr::write_volatile(CLINT_MTIMECMP_LO, val as u32);
        }
    }
    fn update_next_irq(&self, cs: critical_section::CriticalSection) {
        let alarms = self.alarms.borrow(cs).borrow();
        let mut earliest = u64::MAX;
        for a in alarms.iter() {
            if a.callback.is_some() && a.timestamp < earliest { earliest = a.timestamp; }
        }
        Self::write_mtimecmp(if earliest == u64::MAX { u64::MAX } else { earliest.saturating_mul(TICKS_PER_US) });
    }
    pub fn on_timer_interrupt(&self) {
        critical_section::with(|cs| {
            let now = self.now();
            let mut alarms = self.alarms.borrow(cs).borrow_mut();
            for a in alarms.iter_mut() {
                if a.timestamp <= now {
                    a.timestamp = u64::MAX;
                    if let Some((cb, ctx)) = a.callback { cb(ctx); }
                }
            }
            drop(alarms);
            self.update_next_irq(cs);
        });
    }
}

impl Driver for ClintDriver {
    fn now(&self) -> u64 { Self::read_mtime() / TICKS_PER_US }
    unsafe fn allocate_alarm(&self) -> Option<AlarmHandle> {
        critical_section::with(|cs| {
            let mut count = self.allocated.borrow(cs).borrow_mut();
            if (*count as usize) < 4 { let id = *count; *count += 1; Some(AlarmHandle::new(id)) } else { None }
        })
    }
    fn set_alarm_callback(&self, alarm: AlarmHandle, callback: fn(*mut ()), ctx: *mut ()) {
        critical_section::with(|cs| { self.alarms.borrow(cs).borrow_mut()[alarm.id() as usize].callback = Some((callback, ctx)); });
    }
    fn set_alarm(&self, alarm: AlarmHandle, timestamp: u64) -> bool {
        critical_section::with(|cs| {
            if timestamp <= self.now() { return false; }
            {
                let mut alarms = self.alarms.borrow(cs).borrow_mut();
                alarms[alarm.id() as usize].timestamp = timestamp;
            }
            self.update_next_irq(cs);
            true
        })
    }
}

embassy_time_driver::time_driver_impl!(static DRIVER: ClintDriver = ClintDriver::new());

#[no_mangle]
pub extern "C" fn MachineTimer() {
    DRIVER.on_timer_interrupt();
    unsafe {
        let mut mepc: usize;
        core::arch::asm!("csrr {}, mepc", out(reg) mepc);
        core::arch::asm!("csrw mepc, {}", in(reg) mepc);
        core::arch::asm!("mret");
    }
}

pub fn init_embassy_timer() {
    ClintDriver::write_mtimecmp(u64::MAX);
    unsafe {
        let handler_addr = (MachineTimer as *const () as usize) & !0x3;
        core::arch::asm!("csrw mtvec, {}", in(reg) handler_addr);
        riscv::register::mie::set_mtimer();
        riscv::interrupt::enable();
    }
}
