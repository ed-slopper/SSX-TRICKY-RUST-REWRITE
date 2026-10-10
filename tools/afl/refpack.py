def unpack(s):
    flags=s[0]; big=flags&0x80; w=4 if big else 3
    pos=2; size=int.from_bytes(s[pos:pos+w],'big'); pos+=w
    if flags&1: pos+=w
    out=bytearray()
    while pos<len(s):
        b0=s[pos]
        if b0<0x80:
            b1=s[pos+1]; pos+=2; plain=b0&3; dist=((b0&0x60)<<3)+b1+1; cnt=((b0>>2)&7)+3
        elif b0<0xC0:
            b1,b2=s[pos+1],s[pos+2]; pos+=3; plain=b1>>6; dist=((b1&0x3f)<<8)+b2+1; cnt=(b0&0x3f)+4
        elif b0<0xE0:
            b1,b2,b3=s[pos+1],s[pos+2],s[pos+3]; pos+=4; plain=b0&3; dist=((b0&0x10)<<12)+(b1<<8)+b2+1; cnt=((b0&0x0c)<<6)+b3+5
        elif b0<0xFC:
            pos+=1; plain=((b0&0x1f)<<2)+4; out+=s[pos:pos+plain]; pos+=plain; continue
        else:
            pos+=1; plain=b0&3; out+=s[pos:pos+plain]; break
        out+=s[pos:pos+plain]; pos+=plain
        for _ in range(cnt): out.append(out[-dist])
    return bytes(out), size
