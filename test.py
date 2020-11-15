import pymavlink as mv
from pymavlink.dialects.v20 import ardupilotmega as mavlink2
from pymavlink import mavutil as mu

test = """fd 2b 00 00
        5f ff 00 86 00 00 43 87 ed ea 67 0f ec 58 64 00
        3e 02 45 02 4a 02 4c 02 3e 02 44 02 47 02 48 02
        3d 02 41 02 45 02 45 02 41 02 43 02 46 02 48 02
        2a ea d4""";

test = [int(b, base=16) for b in test.split()]

print(test)

class fifo(object):
    def __init__(self):
        self.buf = []
    def write(self, data):
        self.buf += data
        return len(data)
    def read(self):
        return self.buf.pop(0)

buf = bytearray(test)
# buf = test

print(buf[7:10])
print((buf[9] << 16) | (buf[8] << 8) | buf[7])
mav = mavlink2.MAVLink(fifo())
msg = mav.decode(buf)
print(msg)
