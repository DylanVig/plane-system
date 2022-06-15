#!/bin/python

"""
This script will POST all images from DIR_NAME to the ground server running
at GS_URL.

"""

from datetime import datetime
import os
import requests
import json
import argparse

parser = argparse.ArgumentParser()
parser.add_argument(
  "-d", "--directory",
  help="image directory", 
  default='images'
)
parser.add_argument(
  "-g", "--gs-host",
  help="ground server hostname and port", 
  default='127.0.0.1:9000'
)
parser.add_argument(
    "-m", "--mode",
    help="telemetry mode, can be 'dummy' or 'json'",
    default='json'
)
args = parser.parse_args()

DIR_NAME = args.directory
GS_HOST = args.gs_host
TELEM_MODE = args.mode

# Constants for default
IMAGE_LAT = 38.315946
IMAGE_LONG = -76.558576
IMAGE_ALT = 100

# Gather set of each image identifier stored on SD card

ids = set()
for filename in os.scandir(DIR_NAME):
    if filename.is_file():
        ids.add(filename.path.split(".")[0])

print(f'found files: {", ".join(ids)}')

# Post request for each image to gs

upload_log_file = open("save-data-upload.log", "w")

for offset, img_name in enumerate(ids):
    json_filename = img_name + ".json"
    jpg_filename = img_name + ".JPG"

    if TELEM_MODE == 'dummy':
        request_data = {
            "timestamp": int(datetime.now().timestamp()) + offset,
            "imgMode": "fixed",
            "telemetry": {
                "altitude": IMAGE_ALT,
                "planeYaw": 0,
                "gps": {"latitude": IMAGE_LAT, "longitude": IMAGE_LONG},
                "gimOrt": {"pitch": 0, "roll": 0},
            },
        }
    else:
        with open(json_filename, "rb") as json_file:
            request_data = json.load(json_file)

            # {
            #   "plane_attitude": {
            #     "roll": -6.706402,
            #     "pitch": -1.9618407,
            #     "yaw": -163.51987
            #   },
            #   "gimbal_attitude": {
            #     "roll": 0.0,
            #     "pitch": 0.0,
            #     "yaw": 0.0
            #   },
            #   "position": {
            #     "latitude": 42.443626,
            #     "longitude": -76.44113,
            #     "altitude_msl": 291.62,
            #     "altitude_rel": 0.048
            #   },
            #   "time": "2022-02-19T16:37:45.434398765-05:00"
            # }

            request_data = {
              "timestamp": int(datetime.fromisoformat(request_data['telemetry']['time'][:26]).timestamp()),
              "imgMode": "fixed",
              "telemetry": {
                "planeYaw": request_data['telemetry']['plane_attitude']['yaw'],
                "altitude": request_data['telemetry']['position']['altitude_rel'],
                "gps": {
                  "latitude": request_data['telemetry']['position']['point']['x'],
                  "longitude": request_data['telemetry']['position']['point']['y'],
                },
                "gimOrt": { 
                  # NOTE(ibiyemi): no gimbal in bartholomew, so we are using plane pitch and roll for comp 2022
                  "pitch": request_data['telemetry']['plane_attitude']['pitch'], 
                  "roll": request_data['telemetry']['plane_attitude']['roll'],
                },
              }
            }

            print(request_data)

    files = {"json": json.dumps(request_data), "files": open(jpg_filename, "rb")}

    print(f'uploading {jpg_filename} ... ', end = '', flush = True)

    response = requests.post(url=f"http://{GS_HOST}/api/v1/image", files=files)

    if response.status_code == 200:
      print('success')
    else:
        print(f'error ({response.status_code})')
        try:
            response_json = response.json()
            print(f"\tresponse: {response_json}")
        except:
            print(f"\tresponse: {response.text}")

    upload_log_file.write(
        f"file {jpg_filename} status {response.status_code} response '{response.content}'"
    )
    upload_log_file.write("\n")

print("full log written to 'save-data-upload.log'")
