import socket 
import errno
import sys


class Port: 
    def __init__(self, port_number, host):
        self.port_number = port_number
        self.host = host

    def check_port(self):
        socket1 = socket.socket(socket.AF_INET, socket.SOCK_STREAM)
        socket1.settimeout(2.0)
        result = socket1.connect_ex((self.host, self.port_number))

        if result == 0:
            return {"Status": "OPEN", "HOST": self.host, "PORT": self.port_number}
        
        elif result == errno.ECONNREFUSED:
            return {"Status": "CLOSED", "HOST": self.host, "PORT": self.port_number}
        
        elif result == errno.ETIMEDOUT:
            return {"Status": "ERROR", "HOST": self.host, "PORT": self.port_number}
        
        else:
            print(f"ERROR: {result}")
        
        socket1.close()

    def check_connection_protocol(self, timeout=2.0):
        tcp = False
        udp = False

        with socket.socket(socket.AF_INET, socket.SOCK_STREAM) as tcp_socket:
            tcp_socket.settimeout(timeout)
            result = tcp_socket.connect_ex((self.port_number, self.host))
            