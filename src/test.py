import pymavlink as mv
from pymavlink.dialects.v20 import ardupilotmega as mavlink2
from pymavlink import mavutil as mu
test = [253, 146, 11, 254, 14, 243, 1, 1, 29, 3, 35, 5, 0, 148, 66, 108, 68, 10, 215, 35, 54, 171, 13, 193, 228, 254, 14, 244, 1, 1, 137, 3, 35, 5, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 100, 214, 254, 30, 245, 1, 1, 24, 240, 150, 15, 20, 0, 0, 0, 0, 158, 254, 235, 234, 203, 206, 232, 88, 154, 233, 8, 0, 121, 0, 200, 0, 0, 0, 0, 0, 6, 10, 198, 34, 254, 12, 246, 1, 1, 2, 88, 88, 63, 29, 138, 176, 5, 0, 3, 35, 5, 0, 215, 98, 254, 28, 247, 1, 1, 163, 206, 211, 40, 187, 161, 102, 41, 187, 70, 35, 40, 187, 0, 0, 0, 0, 0, 0, 0, 0, 158, 241, 228, 58, 44, 190, 124, 58, 108, 152, 254, 44, 248, 1, 1, 164, 0, 0, 0, 0, 0, 0, 0, 0, 232, 53, 250, 189, 0, 0, 0, 0, 0, 0, 0, 0, 10, 232, 28, 193]

class fifo(object):
    def __init__(self):
        self.buf = []
    def write(self, data):
        self.buf += data
        return len(data)
    def read(self):
        return self.buf.pop(0)

buf = bytes(test)

print(buf[7:10])
print((buf[9] << 16) | (buf[8] << 8) | buf[7])
mav = mavlink2.MAVLink(fifo())
msg = mav.decode(buf)
print(msg)
