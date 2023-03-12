from PIL import Image
import os

directory = 'images'
for filename in os.listdir(directory):
    if filename.split(".")[1] == "json":
        continue
    
    i = os.path.join(directory, filename)
    image = Image.open(i)
    data = list(image.getdata())
    image_without_exif = Image.new(image.mode, image.size)
    image_without_exif.putdata(data)
    
    os.remove(i)
    image_without_exif.save(i)

    print("removed exif from " + i)