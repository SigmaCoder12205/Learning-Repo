# {STATUS: "OPen", }
# 65535
from code.python.port import Port

HOST="127.0.0.1"

current_port = 0
port_open_count = 0
record_of_all_ports = {
    'Well-Known-Ports': [],
    'Registered-Ports': [],
    'Dynamic-Ports': []
}

while current_port <= 65535:
    port = Port(port_number=current_port, host=HOST)
    port_response = port.check_port()

    if port_response.get("Status") == "OPEN":
        
        if port_response.get("PORT") >= 0 and port_response.get("PORT") <= 1023:

            record_of_all_ports['Well-Known-Ports'].append(port_response)
        
        elif port_response.get("PORT") >= 1024 and port_response.get("PORT") <= 49151:

            record_of_all_ports['Registered-Ports'].append(port_response)
        
        else:
            record_of_all_ports['Dynamic-Ports'].append(port_response)

        port_open_count += 1

    current_port += 1
    del port

counter = 0
for port_name, ports in record_of_all_ports.items():
    
    print(f"------------- {port_name} -------------")
    if not ports:
        print("No ports running withn this port range")
    
    for port in ports:
        counter += 1
        print(f"{counter}. {port}")
    
    counter = 0


print(f"\n\nNUMBER OF PORTS OPEN: {port_open_count}")


# ------------------------------------------
# {"Well-Known Ports": "ports": {"Host": ..., "PORT": ..., "STATUS": ...},
#  ""}
# 
# 