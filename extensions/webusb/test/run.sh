cd test_util/raw-gadget/
cd dummy_hcd/
make
./insmod.sh
cd ../raw_gadget/
make
./insmod.sh
cd ../tests/
make
sudo ./gadget &
cd ../../../extensions/webusb/test
python detach_driver.py
