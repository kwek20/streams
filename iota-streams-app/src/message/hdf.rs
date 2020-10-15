use anyhow::{
    ensure,
    Result,
};
use core::fmt;

use iota_streams_core::{
    format,
    sponge::prp::PRP,
};
use iota_streams_ddml::{
    command::*,
    io,
    types::{
        typenum::{
            U2,
            U3,
        },
        *,
    },
};

use super::*;

pub const FLAG_BRANCHING_MASK: u8 = 1;

#[derive(Clone)]
pub struct HDF<Link> {
    pub encoding: Uint8,
    pub version: Uint8,
    // message type is 4 bits
    pub content_type: u8,
    // payload length is 10 bits
    pub payload_length: usize,
    pub frame_type: Uint8,
    // frame count is 22 bits
    pub payload_frame_count: usize,
    pub link: Link,
    pub seq_num: Uint64,
}

impl<Link> HDF<Link> {
    pub fn new(link: Link) -> Self {
        Self {
            encoding: UTF8,
            version: STREAMS_1_VER,
            content_type: 0,
            payload_length: 0,
            frame_type: HDF_ID,
            payload_frame_count: 0,
            link: link,
            seq_num: Uint64(0),
        }
    }

    pub fn with_content_type(mut self, content_type: u8) -> Result<Self> {
        ensure!(content_type < 0x10, "Content type out of range: {}", content_type);
        self.content_type = content_type;
        Ok(self)
    }

    pub fn get_content_type(&self) -> u8 {
        self.content_type
    }

    pub fn with_payload_length(mut self, payload_length: usize) -> Result<Self> {
        ensure!(
            payload_length < 0x0400,
            "Payload length out of range: {}",
            payload_length
        );
        self.payload_length = payload_length;
        Ok(self)
    }

    pub fn get_payload_length(&self) -> usize {
        self.payload_length
    }

    pub fn with_payload_frame_count(mut self, payload_frame_count: usize) -> Result<Self> {
        ensure!(
            payload_frame_count < 0x400000,
            "Payload frame count out of range: {}",
            payload_frame_count
        );
        self.payload_frame_count = payload_frame_count;
        Ok(self)
    }

    pub fn get_payload_frame_count(&self) -> usize {
        self.payload_frame_count
    }

    pub fn with_seq_num(mut self, seq_num: u32) -> Self {
        self.seq_num = Uint64(seq_num as u64);
        self
    }

    pub fn get_seq_num(&self) -> u64 {
        self.seq_num.0
    }

    pub fn new_with_fields(link: Link, content_type: u8, payload_length: usize, seq_num: u64) -> Result<Self> {
        ensure!(content_type < 0x10, "Content type out of range: {}", content_type);
        ensure!(
            payload_length < 0x0400,
            "Payload length out of range: {}",
            payload_length
        );
        Ok(Self {
            encoding: UTF8,
            version: STREAMS_1_VER,
            content_type,
            payload_length,
            frame_type: HDF_ID,
            payload_frame_count: 0,
            link: link,
            seq_num: Uint64(seq_num),
        })
    }
}

impl<Link: Default> Default for HDF<Link> {
    fn default() -> Self {
        Self {
            encoding: UTF8,
            version: STREAMS_1_VER,
            content_type: 0,
            payload_length: 0,
            frame_type: HDF_ID,
            payload_frame_count: 0,
            link: Link::default(),
            seq_num: Uint64(0),
        }
    }
}

impl<Link> fmt::Debug for HDF<Link>
where
    Link: fmt::Debug,
{
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "{{encoding: {:?}, version: {:?}, content_type: {:?}, payload_length: {:?}}}",
            self.encoding,
            self.version,
            self.get_content_type(),
            self.get_payload_length()
        )
    }
}

impl<F, Link> ContentSizeof<F> for HDF<Link>
where
    F: PRP,
    Link: AbsorbExternalFallback<F>,
{
    fn sizeof<'c>(&self, ctx: &'c mut sizeof::Context<F>) -> Result<&'c mut sizeof::Context<F>> {
        let content_type_and_payload_length = NBytes::<U2>::default();
        let payload_frame_count = NBytes::<U3>::default();
        ctx.absorb(self.encoding)?
            .absorb(self.version)?
            .skip(&content_type_and_payload_length)?
            .absorb(External(Uint8(self.content_type << 4)))?
            .absorb(self.frame_type)?
            .skip(&payload_frame_count)?
            .absorb(External(Fallback(&self.link)))?
            .skip(self.seq_num)?;
        Ok(ctx)
    }
}

impl<F, Link, Store> ContentWrap<F, Store> for HDF<Link>
where
    F: PRP,
    Link: AbsorbExternalFallback<F>,
{
    fn wrap<'c, OS: io::OStream>(
        &self,
        _store: &Store,
        ctx: &'c mut wrap::Context<F, OS>,
    ) -> Result<&'c mut wrap::Context<F, OS>> {
        let content_type_and_payload_length = {
            let mut nbytes = NBytes::<U2>::default();
            let v = nbytes.as_mut();
            v[0] = (self.content_type << 4) | ((self.payload_length >> 8) as u8 & 0x03);
            v[1] = self.payload_length as u8;
            nbytes
        };
        let payload_frame_count = {
            let mut nbytes = NBytes::<U3>::default();
            let v = nbytes.as_mut();
            let x = self.payload_frame_count.to_be_bytes();
            
            destruct_usize(v, x);
            nbytes
        };

        ctx.absorb(self.encoding)?
            .absorb(self.version)?
            .skip(&content_type_and_payload_length)?
            .absorb(External(Uint8(self.content_type << 4)))?
            .absorb(self.frame_type)?
            .skip(&payload_frame_count)?
            .absorb(External(Fallback(&self.link)))?
            .skip(self.seq_num)?;
        Ok(ctx)
    }
}

impl<F, Link, Store> ContentUnwrap<F, Store> for HDF<Link>
where
    F: PRP,
    Link: AbsorbExternalFallback<F>,
{
    fn unwrap<'c, IS: io::IStream>(
        &mut self,
        _store: &Store,
        ctx: &'c mut unwrap::Context<F, IS>,
    ) -> Result<&'c mut unwrap::Context<F, IS>> {
        let mut content_type_and_payload_length = NBytes::<U2>::default();
        let mut payload_frame_count = NBytes::<U3>::default();

        ctx.absorb(&mut self.encoding)?
            .absorb(&mut self.version)?
            .guard(
                self.version == STREAMS_1_VER,
                &format!(
                    "Message version not supported: expected {}, found {}.",
                    STREAMS_1_VER, self.version
                ),
            )?
            .skip(&mut content_type_and_payload_length)?;
        {
            let v = content_type_and_payload_length.as_ref();
            ensure!(0 == v[0] & 0x0c, "Bad reserved bits");
            self.content_type = v[0] >> 4;
            self.payload_length = (((v[0] & 0x03) as usize) << 8) | (v[1] as usize);
        }

        ctx.absorb(External(Uint8(self.content_type << 4)))?
            .absorb(&mut self.frame_type)?
            .guard(
                self.frame_type == HDF_ID,
                &format!(
                    "Message frame type not supported: expected {}, found {}.",
                    HDF_ID, self.frame_type
                ),
            )?
            .skip(&mut payload_frame_count)?;
        {
            let v = payload_frame_count.as_ref();
            ensure!(0 == v[0] & 0xc0, "Bad reserved bits");
            self.payload_frame_count = construct_usize(v);
        }

        ctx.absorb(External(Fallback(&self.link)))?.skip(&mut self.seq_num)?;

        Ok(ctx)
    }
}

#[cfg(target_pointer_width = "32")]
fn construct_usize(v: &[u8]) -> usize{
    let mut x = [0_u8; 4];
    x[1] = v[0];
    x[2] = v[1];
    x[3] = v[2];
    usize::from_be_bytes(x)
}

#[cfg(target_pointer_width = "64")]
fn construct_usize(v: &[u8]) -> usize{
    let mut x = [0_u8; 8];
    x[5] = v[0];
    x[6] = v[1];
    x[7] = v[2];
    usize::from_be_bytes(x)
}

#[cfg(target_pointer_width = "32")]
fn destruct_usize(v: &[u8], x: u8[8]){
    v[0] = x[1] & 0x3f;
    v[1] = x[2];
    v[2] = x[3];
}

#[cfg(target_pointer_width = "64")]
fn destruct_usize(v: &[u8],  x: u8[8]){
    v[0] = x[5] & 0x3f;
    v[1] = x[6];
    v[2] = x[7];
}
