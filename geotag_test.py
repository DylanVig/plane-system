#!/bin/python

"""
This script will calculate the geotag of the ROI for a given image name, focal
length, pixel x, and pixel y.
"""

import os
import json
import argparse
from math import cos, sin, tan, asin, atan, atan2, sqrt, pi
import numpy as np

# in pixels, constant for the R10C
image_width = 5456
image_height = 3632

# in mm, constant for the R10C
sensor_width = 23.2
sensor_height = 15.4

parser = argparse.ArgumentParser()
parser.add_argument(
  "-i", "--image",
  help="image name", 
  default='DSC00256.JPG'
)
parser.add_argument(
  "-f", "--focallength",
  help="focal length of camera", 
  default='16'
)
parser.add_argument(
  "-x", "--pixelx",
  help="x pixel from left of image", 
  default=str(image_width / 2)
)
parser.add_argument(
  "-y", "--pixely",
  help="y pixel from top of image", 
  default=str(image_height / 2)
)
args = parser.parse_args()

IMAGE_NAME = args.image
FOCAL_LENGTH = int(args.focallength)
PIXEL_X = int(args.pixelx)
PIXEL_Y = int(args.pixely)


# FOV in radians
fov_x = 2 * atan(sensor_width / (2 * FOCAL_LENGTH))
fov_y = 2 * atan(sensor_height / (2 * FOCAL_LENGTH))

json_filename = os.path.join('images', IMAGE_NAME + ".json")
jpg_filename = os.path.join('images', IMAGE_NAME + ".JPG")

with open(json_filename, "rb") as json_file:
  telem = json.load(json_file)

  image_lat = telem['pixhawk']['position'][0]['point']['lat']
  image_long = telem['pixhawk']['position'][0]['point']['lon']
  image_alt_msl = telem['pixhawk']['position'][0]['altitude_msl']
  image_altitude = telem['pixhawk']['position'][0]['altitude_rel']

  image_roll = telem['pixhawk']['attitude'][0]['roll']
  image_pitch = telem['pixhawk']['attitude'][0]['pitch']
  image_yaw = telem['pixhawk']['attitude'][0]['yaw']




# pixels (x, y) from origin being center, and positive being in (right, up) direction
delta_pixel_x = PIXEL_X - image_width / 2
delta_pixel_y = image_height / 2 - PIXEL_Y


""" GEOTAGGING CALCULATIONS: do not change unless you want to edit the model """

# Calculate target_dx and target_dy
# target_dx = meters between target and plane on ground in longitude/x direction
# target_dy = meters between target and plane on ground in latitude/y direction

# New approach

# Focal length in pixels
f_pixels = FOCAL_LENGTH * image_width / sensor_width

# Unit vector of target wrt plane in P frame
r_unit_target_rel_plane_P = 1 / sqrt(f_pixels ** 2 + delta_pixel_x ** 2 + delta_pixel_y ** 2) * np.array([delta_pixel_x, delta_pixel_y, -f_pixels])

# Calculate DCM - Note that yaw is negated because reference frame definition would indicate positive is CC but positive yaw is clockwise
C_2_roll = np.matrix([[cos(image_roll), 0, -sin(image_roll)], [0, 1, 0], [sin(image_roll), 0, cos(image_roll)]])
C_1_pitch = np.matrix([[1, 0, 0], [0, cos(image_pitch), sin(image_pitch)], [0, -sin(image_pitch), cos(image_pitch)]])
C_3_yaw = np.matrix([[cos(-image_yaw), sin(-image_yaw), 0], [-sin(-image_yaw), cos(-image_yaw), 0], [0, 0, 1]])
P_C_I = C_2_roll @ C_1_pitch @ C_3_yaw
I_C_P = np.transpose(P_C_I)

# Transform into unit vector of target wrt plane in I frame
r_unit_target_rel_plane_I = np.transpose(I_C_P @ r_unit_target_rel_plane_P)

# Find actual vector of target wrt plane; first and second entries are target_dx and target_dy
r_target_plane_I = image_altitude / abs(r_unit_target_rel_plane_I[2, 0]) * r_unit_target_rel_plane_I
target_dx = r_target_plane_I[0, 0]
target_dy = r_target_plane_I[1, 0]


# Old approach
target_dx_old = image_altitude * (
							tan(-image_roll + fov_x * delta_pixel_x / image_width) * cos(image_yaw)
        			  	  + tan(image_pitch + fov_y * delta_pixel_y / image_height) * sin(image_yaw)
        			   )
target_dy_old = image_altitude * (
							tan(-image_roll + fov_x * delta_pixel_x / image_width) * sin(-image_yaw)
        			  	  + tan(image_pitch + fov_y * delta_pixel_y / image_height) * cos(image_yaw)
        			   )


print(image_roll, image_pitch, image_yaw)
print(target_dx, target_dx_old)
print(target_dy, target_dy_old)

# Computations for conversion back into lat/long using inverse haversine
distance_to_target = sqrt(target_dx_old ** 2 + target_dy ** 2) # meters
direction_to_target = pi / 2.0 - atan2(target_dy, target_dx) # radians

# Returns new latitude and longitude in DEGREES
def inverse_haversine(ilat, ilong, dist, dir):
	r = 6371000.0 # approximate radius of Earth in meters

	new_lat = 180 / pi * asin(sin(ilat) * cos(dist / r) + cos(ilat) * sin(dist / r) * cos(dir))
	new_long = 180 / pi * (ilong + atan2(sin(dir) * sin(dist / r) * cos(ilat), cos(dist / r) - sin(ilat) * sin(new_lat)))

	return new_lat, new_long

# new gps in radians
new_lat, new_long = inverse_haversine(image_lat * pi / 180, image_long * pi / 180, distance_to_target, direction_to_target)

# Prints the predicted lat/long of the image
print(new_lat, image_lat, new_lat - image_lat)
print(new_long, image_long, new_long - image_long)
