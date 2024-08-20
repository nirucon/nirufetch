CC = gcc
CFLAGS = -Wall -O2

all: nirufetch

nirufetch: nirufetch.o
	$(CC) $(CFLAGS) -o nirufetch nirufetch.o

nirufetch.o: nirufetch.c
	$(CC) $(CFLAGS) -c nirufetch.c

clean:
	rm -f nirufetch.o nirufetch

install: nirufetch
	install -m 755 nirufetch /usr/local/bin/

uninstall:
	rm -f /usr/local/bin/nirufetch
