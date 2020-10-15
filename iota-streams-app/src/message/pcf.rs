use anyhow::{
    ensure,
    Result,
};

use iota_streams_core::sponge::prp::PRP;
use iota_streams_ddml::{
    command::*,
    io,
    types::{
        typenum::U3,
        GenericArray,
        *,
    },
};

use super::*;

pub struct PCF<Content> {
    pub frame_type: Uint8,
    // 22-bit field
    pub payload_frame_num: NBytes<U3>,
    pub content: Content,
}

impl PCF<()> {
    pub fn new_init_frame() -> Self {
        Self {
            frame_type: INIT_PCF_ID,
            payload_frame_num: NBytes::default(),
            content: (),
        }
    }

    pub fn new_inter_frame() -> Self {
        Self {
            frame_type: INTER_PCF_ID,
            payload_frame_num: NBytes::default(),
            content: (),
        }
    }

    pub fn new_final_frame() -> Self {
        Self {
            frame_type: FINAL_PCF_ID,
            payload_frame_num: NBytes::default(),
            content: (),
        }
    }

    pub fn with_content<Content>(self, content: Content) -> PCF<Content> {
        PCF {
            frame_type: self.frame_type,
            payload_frame_num: self.payload_frame_num,
            content: content,
        }
    }
}

fn payload_frame_num_from(n: usize) -> Result<NBytes<U3>> {
    ensure!(n < 0x400000, "Payload frame num out of range: {}", n);
    let v = n.to_be_bytes();
    let g = <GenericArray<u8, U3>>::from_slice(&v[5..]);
    Ok(NBytes::from(*g))
}

#[cfg(target_pointer_width = "32")]
fn payload_frame_num_to(v: &NBytes<U3>) -> usize {
    let mut u = [0_u8; 4];
    u[1..].copy_from_slice(v.as_ref());
    usize::from_be_bytes(u)
}

#[cfg(target_pointer_width = "64")]
fn payload_frame_num_to(v: &NBytes<U3>) -> usize {
    let mut u = [0_u8; 8];
    u[5..].copy_from_slice(v.as_ref());
    usize::from_be_bytes(u)
}

fn payload_frame_num_check(v: &NBytes<U3>) -> Result<()> {
    ensure!(v.as_ref()[0] < 0x40, "Payload frame num out of range");
    Ok(())
}

impl<Content> PCF<Content> {
    pub fn new(frame_type: Uint8, payload_frame_num: usize, content: Content) -> Result<Self> {
        payload_frame_num_from(payload_frame_num).map(|payload_frame_num| Self {
            frame_type,
            payload_frame_num,
            content,
        })
    }

    pub fn with_payload_frame_num(mut self, payload_frame_num: usize) -> Result<Self> {
        payload_frame_num_from(payload_frame_num).map(|payload_frame_num| {
            self.payload_frame_num = payload_frame_num;
            self
        })
    }

    pub fn default_with_content(content: Content) -> Self {
        let v = [0, 0, 1_u8];
        let payload_frame_num = NBytes::from(GenericArray::from(v));
        Self {
            frame_type: FINAL_PCF_ID,
            payload_frame_num,
            content,
        }
    }

    pub fn get_payload_frame_num(&self) -> usize {
        payload_frame_num_to(&self.payload_frame_num)
    }
}

impl<F, Content> ContentSizeof<F> for PCF<Content>
where
    F: PRP,
    Content: ContentSizeof<F>,
{
    fn sizeof<'c>(&self, mut ctx: &'c mut sizeof::Context<F>) -> Result<&'c mut sizeof::Context<F>> {
        ctx.absorb(&self.frame_type)?.skip(&self.payload_frame_num)?;
        self.content.sizeof(&mut ctx)?;
        Ok(ctx)
    }
}

impl<F, Content, Store> ContentWrap<F, Store> for PCF<Content>
where
    F: PRP,
    Content: ContentWrap<F, Store>,
{
    fn wrap<'c, OS: io::OStream>(
        &self,
        store: &Store,
        mut ctx: &'c mut wrap::Context<F, OS>,
    ) -> Result<&'c mut wrap::Context<F, OS>> {
        ctx.absorb(&self.frame_type)?.skip(&self.payload_frame_num)?;
        self.content.wrap(store, &mut ctx)?;
        Ok(ctx)
    }
}

impl<F, Content, Store> ContentUnwrap<F, Store> for PCF<Content>
where
    F: PRP,
    Content: ContentUnwrap<F, Store>,
{
    fn unwrap<'c, IS: io::IStream>(
        &mut self,
        store: &Store,
        mut ctx: &'c mut unwrap::Context<F, IS>,
    ) -> Result<&'c mut unwrap::Context<F, IS>> {
        ctx.absorb(&mut self.frame_type)?.skip(&mut self.payload_frame_num)?;
        payload_frame_num_check(&self.payload_frame_num)?;
        self.content.unwrap(&store, &mut ctx)?;
        Ok(ctx)
    }
}
