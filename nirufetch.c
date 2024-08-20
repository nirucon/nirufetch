/*
 * nirufetch.c - A simple system information fetcher inspired by other *fetch scripts, using Font Awesome icons.
 * OS support: only Arch Linux at the moment...
 * Version: 0.1
 * Author: Nicklas Rudolfsson https://github.com/nirucon
 * License: None = FREE
 *
 * Dependencies:
 * - Font Awesome (for icons)
 * - GNU Core Utilities (uname, df, etc.)
 * - Pacman (for counting installed packages)
 * - curl (for fetching public IP)
 * - iproute2 (for fetching local IP)
 * 
 * To build and install:
 * 1. Save this file as `nirufetch.c`.
 * 2. Include the `Makefile`.
 * 3. Run `sudo make clean install` in the terminal.
 * 4. Use the `nirufetch` command to display system information with icons.
 */

#include <stdio.h>
#include <stdlib.h>
#include <string.h>
#include <unistd.h>
#include <sys/utsname.h>

void print_info(const char *icon, const char *text) {
    printf("%s %s\n", icon, text);
}

void get_hostname() {
    char hostname[256];
    char username[256];
    FILE *fp = fopen("/etc/hostname", "r");
    if (fp == NULL) {
        perror("fopen");
        exit(EXIT_FAILURE);
    }
    fgets(hostname, sizeof(hostname), fp);
    hostname[strcspn(hostname, "\n")] = 0;  // Remove newline character
    fclose(fp);

    getlogin_r(username, sizeof(username)); // Get the username
    char user_host[512];
    snprintf(user_host, sizeof(user_host), "%s@%s", username, hostname); // Combine username and hostname

    print_info("\uf015  ", user_host);
}

void get_os() {
    struct utsname buffer;
    if (uname(&buffer) != 0) {
        perror("uname");
        exit(EXIT_FAILURE);
    }

    FILE *fp = fopen("/etc/os-release", "r");
    char distro_name[256] = "Unknown Distro";

    if (fp != NULL) {
        char line[256];
        while (fgets(line, sizeof(line), fp)) {
            if (strncmp(line, "PRETTY_NAME=", 12) == 0) {
                strncpy(distro_name, strchr(line, '=') + 2, sizeof(distro_name) - 1);
                distro_name[strcspn(distro_name, "\"\n")] = 0; // Remove quotation marks and newline
                break;
            }
        }
        fclose(fp);
    }

    char os_info[512];
    snprintf(os_info, sizeof(os_info), "%s@%s %s %s", distro_name, buffer.sysname, buffer.release, buffer.machine); // Combine distro and kernel

    print_info("\uf17c  ", os_info);
}

void get_uptime() {
    FILE *uptime_file = fopen("/proc/uptime", "r");
    if (uptime_file == NULL) {
        perror("fopen");
        exit(EXIT_FAILURE);
    }
    
    double uptime_seconds;
    fscanf(uptime_file, "%lf", &uptime_seconds);
    fclose(uptime_file);

    int days = uptime_seconds / 86400;
    int hours = (uptime_seconds / 3600) - (days * 24);
    int minutes = (uptime_seconds / 60) - (days * 1440) - (hours * 60);
    
    char uptime_info[256];
    snprintf(uptime_info, sizeof(uptime_info), "%d days, %d hours, %d minutes", days, hours, minutes);
    print_info("\uf017  ", uptime_info);
}

void get_packages() {
    FILE *fp;
    int count = 0;
    char path[1035];

    fp = popen("pacman -Qq | wc -l", "r");
    if (fp == NULL) {
        printf("Failed to run command\n");
        exit(EXIT_FAILURE);
    }

    while (fgets(path, sizeof(path), fp) != NULL) {
        count = atoi(path);
    }

    char package_info[256];
    snprintf(package_info, sizeof(package_info), "%d (pacman)", count);
    print_info("\uf187  ", package_info);
    pclose(fp);
}

void get_shell() {
    char *shell = getenv("SHELL");
    print_info("\uf120  ", shell);
}

void get_cpu() {
    FILE *cpuinfo = fopen("/proc/cpuinfo", "r");
    if (cpuinfo == NULL) {
        perror("fopen");
        exit(EXIT_FAILURE);
    }

    char buffer[256];
    while (fgets(buffer, sizeof(buffer), cpuinfo)) {
        if (strncmp(buffer, "model name", 10) == 0) {
            char *model_name = strchr(buffer, ':') + 2;
            model_name[strcspn(model_name, "\n")] = 0;  // Remove newline character
            print_info("\uf2db  ", model_name);
            break;
        }
    }
    fclose(cpuinfo);
}

void get_memory() {
    FILE *meminfo = fopen("/proc/meminfo", "r");
    if (meminfo == NULL) {
        perror("fopen");
        exit(EXIT_FAILURE);
    }

    char buffer[256];
    long total_memory = 0;
    long free_memory = 0;

    while (fgets(buffer, sizeof(buffer), meminfo)) {
        if (strncmp(buffer, "MemTotal", 8) == 0) {
            sscanf(buffer, "MemTotal: %ld kB", &total_memory);
        } else if (strncmp(buffer, "MemAvailable", 12) == 0) {
            sscanf(buffer, "MemAvailable: %ld kB", &free_memory);
            break;
        }
    }

    fclose(meminfo);

    long used_memory = total_memory - free_memory;

    char memory_info[256];
    snprintf(memory_info, sizeof(memory_info), "%.2fGi / %.2fGi", used_memory / 1024.0 / 1024.0, total_memory / 1024.0 / 1024.0);
    print_info("\uf538  ", memory_info);
}

void get_disk() {
    FILE *fp;
    char used[16], size[16];

    fp = popen("df -h --output=used,size / | tail -1", "r");
    if (fp == NULL) {
        printf("Failed to run command\n");
        exit(EXIT_FAILURE);
    }

    fscanf(fp, "%15s %15s", used, size);
    pclose(fp);

    char disk_info[256];
    snprintf(disk_info, sizeof(disk_info), "%5s / %s", used, size);
    print_info("\uf0a0", disk_info);
}

void get_ip() {
    FILE *fp;
    char local_ip[1035];
    char public_ip[1035];

    // Get local IP
    fp = popen("ip -4 addr show | grep -oP '(?<=inet\\s)\\d+(\\.\\d+){3}' | grep -v '127.0.0.1'", "r");
    if (fp == NULL) {
        printf("Failed to run command\n");
        exit(EXIT_FAILURE);
    }

    if (fgets(local_ip, sizeof(local_ip), fp) != NULL) {
        local_ip[strcspn(local_ip, "\n")] = 0;  // Remove newline character
        print_info("\uf0ac  ", local_ip);
    } else {
        printf("Local IP:    Not available\n");
    }
    pclose(fp);

    // Get public IP
    fp = popen("curl -s ifconfig.me", "r");
    if (fp == NULL) {
        printf("Failed to run command\n");
        exit(EXIT_FAILURE);
    }

    if (fgets(public_ip, sizeof(public_ip), fp) != NULL) {
        public_ip[strcspn(public_ip, "\n")] = 0;  // Remove newline character
        print_info("\uf0ac  ", public_ip);
    } else {
        printf("Public IP:    Not available\n");
    }
    pclose(fp);
}

int main() {
    get_hostname();
    get_os();
    get_uptime();
    get_packages();
    get_shell();
    get_cpu();
    get_memory();
    get_disk();
    get_ip();

    return 0;
}

