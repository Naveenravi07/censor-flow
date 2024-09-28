 #!/bin/sh

systemctl --user restart pipewire-pulse.socket

systemctl --user restart pipewire-pulse.service

systemctl --user restart pipewire.service

systemctl --user restart pipewire.socket 
