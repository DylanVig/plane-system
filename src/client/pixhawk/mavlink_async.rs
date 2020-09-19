use crc_any::CRCu16;
use mavlink::{self, MavHeader, MavlinkVersion, Message, MAV_STX_V2};
use smol::{
    io::AsyncRead, io::AsyncReadExt, io::AsyncWrite, io::AsyncWriteExt, prelude::StreamExt,
};

const MAVLINK_IFLAG_SIGNED: u8 = 0x01;

/// Read a MAVLink v2  message from a Read stream.
pub async fn read_v2_msg<M: Message, R: AsyncRead + Unpin>(
    r: &mut R,
) -> anyhow::Result<(MavHeader, M)> {
    loop {
        let mut iter = r.bytes();

        // search for the magic framing value indicating start of mavlink message
        loop {
            if let Some(byte) = iter.next().await {
                match byte {
                    Ok(byte) => {
                        if byte == MAV_STX_V2 {
                            break;
                        }
                    }
                    Err(err) => return Err(err.into()),
                }
            } else {
              return Err(anyhow!("stream ended"));
            }
        }

        let mut header = [0; 9];
        r.read_exact(&mut header[..]).await?;

        let payload_len = header[0] as usize;
        let incompat_flags = header[1];
        let compat_flags = header[2];
        let seq = header[3];
        let sysid = header[4];
        let compid = header[5];

        let mut msgid_buf = [0; 4];
        msgid_buf[0] = header[6];
        msgid_buf[1] = header[7];
        msgid_buf[2] = header[8];

        let msgid: u32 = u32::from_le_bytes(msgid_buf);

        // provide a buffer that is the maximum payload size
        let mut payload = [0; 255];
        r.read_exact(&mut payload[..payload_len]).await?;

        let mut crc = [0; 2];
        r.read_exact(&mut crc[..]).await?;
        let crc = u16::from_le_bytes(crc);

        if (incompat_flags & 0x01) == MAVLINK_IFLAG_SIGNED {
            let mut sign = [0; 13];
            r.read_exact(&mut sign).await?;
        }

        let mut crc_calc = CRCu16::crc16mcrf4cc();
        crc_calc.digest(&header[..]);
        crc_calc.digest(&payload[..]);
        let extra_crc = M::extra_crc(msgid);

        crc_calc.digest(&[extra_crc]);
        let recvd_crc = crc_calc.get_crc();
        if recvd_crc != crc {
            // bad crc: ignore message
            continue;
        }

        return M::parse(MavlinkVersion::V2, msgid, &payload[..])
            .map(|msg| {
                (
                    MavHeader {
                        sequence: seq,
                        system_id: sysid,
                        component_id: compid,
                    },
                    msg,
                )
            })
            .map_err(|err| err.into());
    }
}

/// Write a MAVLink v2 message to a Write stream.
pub async fn write_v2_msg<M: Message, W: AsyncWrite + Unpin>(
    w: &mut W,
    header: MavHeader,
    data: &M,
) -> std::io::Result<()> {
    let msgid = data.message_id();
    let payload = data.ser();

    let header = &[
        MAV_STX_V2,
        payload.len() as u8,
        0, //incompat_flags
        0, //compat_flags
        header.sequence,
        header.system_id,
        header.component_id,
        (msgid & 0x0000FF) as u8,
        ((msgid & 0x00FF00) >> 8) as u8,
        ((msgid & 0xFF0000) >> 16) as u8,
    ];

    let mut crc = CRCu16::crc16mcrf4cc();
    crc.digest(&header[1..]);
    crc.digest(&payload[..]);
    let extra_crc = M::extra_crc(msgid);
    crc.digest(&[extra_crc]);

    w.write_all(header).await?;
    w.write_all(&payload[..]).await?;
    let crc = u16::to_le_bytes(crc.get_crc());
    w.write_all(&crc[..]).await?;

    Ok(())
}
