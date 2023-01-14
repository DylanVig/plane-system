#!/bin/python

"""
This script will calculate the geotag of the ROI at PIXEL_X and PIXEL_Y in image
with IMAGE_NAME.
"""

import os
import json
import argparse
from math import cos, sin, tan, asin, atan, atan2, sqrt, pi

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

print(delta_pixel_x, delta_pixel_y)


""" GEOTAGGING CALCULATIONS: do not change unless you want to edit the model """

hdi = image_altitude * sensor_width / FOCAL_LENGTH
vdi = image_altitude * sensor_height / FOCAL_LENGTH

# meters between target and plane on ground in longitude/x direction 
target_dx = image_altitude * (
							tan(-image_roll + fov_x * delta_pixel_x / image_width) * cos(image_yaw)
        			  	  + tan(image_pitch + fov_y * delta_pixel_y / image_height) * sin(image_yaw)
        			   )

# meters between target and plane on ground in latitude/y direction 
target_dy = image_altitude * (
							tan(-image_roll + fov_x * delta_pixel_x / image_width) * sin(-image_yaw)
        			  	  + tan(image_pitch + fov_y * delta_pixel_y / image_height) * cos(image_yaw)
        			   )

distance_to_target = sqrt(target_dx ** 2 + target_dy ** 2) # meters
direction_to_target = pi / 2.0 - atan2(target_dy, target_dx) # radians

print(distance_to_target)

# Returns new latitude and longitude in DEGREES
def inverse_haversine(ilat, ilong, dist, dir):
	r = 6371000.0 # approximate radius of Earth in meters

	new_lat = 180 / pi * asin(sin(ilat) * cos(dist / r) + cos(ilat) * sin(dist / r) * cos(dir))
	new_long = 180 / pi * (ilong + atan2(sin(dir) * sin(dist / r) * cos(ilat), cos(dist / r) - sin(ilat) * sin(new_lat)))

	return new_lat, new_long

# new gps in radians
new_lat, new_long = inverse_haversine(image_lat * pi / 180, image_long * pi / 180, distance_to_target, direction_to_target)

# Prints the predicted lat/long of the image
print(new_lat)
print(new_long)
