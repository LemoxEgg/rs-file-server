#include <stdlib.h>
#include <ifaddrs.h>
#include <string.h>


unsigned int ip(char inter_name[], unsigned char ip[4]) {
    struct ifaddrs *interfaces;
    
    if (getifaddrs(&interfaces) == -1) {
        exit(EXIT_FAILURE);
    }
    
    for (
        struct ifaddrs *inter = interfaces;
        inter != NULL;
        inter = inter->ifa_next
    ) {
        if (
            inter->ifa_addr->sa_family == AF_INET &&
            strcmp(inter->ifa_name, inter_name) == 0
        ) {
            unsigned char *data = (unsigned char*) inter->ifa_addr->sa_data;
            ip[0] = data[2];
            for (int i = 3; i < 6; i++) {
                ip[i-2] = data[i];
            }
            break;
        }
    }
    freeifaddrs(interfaces);
    return 0;
}

/*
int main(int argc, char const *argv[])
{
    unsigned char ip_array[4];
    ip("wlp4s0", ip_array);
    printf("%d.%d.%d.%d\n", ip_array[0], ip_array[1], ip_array[2], ip_array[3]);
    return 0;
}
*/
