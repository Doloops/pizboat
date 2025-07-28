// Installation:
//      Download: www.airspayce.com/mikem/bcm2835/index.html
//      configure
//      make
//      make install
//      gcc -lbcm2835 C_BCM2835.c
// Speed:
//      5.4 MHz

#include "bcm2835.h"
#include <stdio.h>

#include <unistd.h>
#include <sys/time.h>

#include <time.h>


// #define PIN RPI_GPIO_P1_07 // GPIO 4

#define DOUT 5
#define SCK  6

#define OFFSET 8661777
#define SCALE -960.33

void nslp0()
{
    struct timespec tv;
    tv.tv_sec = 0;
    tv.tv_nsec = 1;
    nanosleep(&tv, NULL);
}

void nslp1()
{
    for (int i = 0 ; i < 1000 ; i++ )
    {
    }
}

void nslp()
{
    nslp1();
}

int main(int argc, char *argv[]) {
    if(!bcm2835_init())
        return 1;
    
    uint8_t r;
    
    fprintf(stderr, "Init\n");
    // Set the pin to be an output
    bcm2835_gpio_fsel(SCK, BCM2835_GPIO_FSEL_OUTP);
    bcm2835_gpio_fsel(DOUT, BCM2835_GPIO_FSEL_INPT);

    bcm2835_gpio_write(SCK, HIGH);
    nslp();
    bcm2835_gpio_write(SCK, LOW);
    nslp();
    
    struct timeval tv;
    gettimeofday(&tv, NULL);
    uint64_t birth = tv.tv_sec;

    int nbframes = 0;

    while(1) {
        
        while(1)
        {
            r = bcm2835_gpio_lev(DOUT);
            // fprintf(stderr, "r %u\n", r);
            if ( r == 0 )
                break;
            usleep(100);
            // nslp();
        }

        int count = 0;
        for ( int i = 0 ; i < 24 ; i++ )
        {
            bcm2835_gpio_write(SCK, HIGH);
            nslp();
            bcm2835_gpio_write(SCK, LOW);
            nslp();
            
            count <<= 1;
            
            r = bcm2835_gpio_lev(DOUT);
            if ( r )
                count ++;
            // fprintf(stderr, "i %d r %u\n", i, r);
        }

        for ( int i = 0 ; i < 1 ; i++ )
        {
            bcm2835_gpio_write(SCK, HIGH);
            nslp();
            bcm2835_gpio_write(SCK, LOW);
            nslp();
        }
        
        count = count ^ 0x800000;
        
        float val = count;
        val -= OFFSET;
        val /= SCALE;
        
        usleep(200000);
        nbframes ++;
    
        gettimeofday(&tv, NULL);
        uint64_t age = tv.tv_sec - birth;
        
        float framerate = age > 0 ? ((float) nbframes/ (float) age) : 0.0f;

        fprintf(stderr, "count=%d, val=%f framerate=%f\n", count, val, framerate);
    }

    return 0;
}
