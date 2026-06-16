			#include<p16f628.inc>

			__CONFIG   _CP_OFF & _DATA_CP_OFF & _LVP_OFF & _BODEN_OFF & _MCLRE_ON & _WDT_OFF & _PWRTE_ON & _INTRC_OSC_NOCLKOUT

UNIDAD		EQU		0x20			
DECENA 		EQU 	0x21			
UNIADDR 	EQU 	0x00
DECADDR 	EQU 	0x01
CONT1		EQU		0x22
CONT2		EQU		0x23
AUXW		EQU		0x24
AUXSTATUS	EQU		0x25
	
			ORG 	0x00
			goto	conf 			
		
			ORG 	0x04 					
			bcf	    INTCON,GIE				
			bcf		INTCON,INTF
			movwf   AUXW           
			movf	STATUS,w          
			movwf	AUXSTATUS       
								;SUMA
			incf	UNIDAD,1			
			movlw	0x0A
			xorwf	UNIDAD,w
			btfss	STATUS,Z
			goto	wr_unidad
			clrf	UNIDAD
			incf	DECENA,1
			movlw	0x0A
			xorwf	DECENA,w
			btfsc	STATUS,Z
			clrf	DECENA			
wr_decena
			movf	DECENA,w						
			bsf		STATUS,RP0		
			movwf	EEDATA				
			movlw	DECADDR				
			movwf	EEADR			
			call	wr_eeprom		
			bcf		STATUS,RP0

wr_unidad
			movf	UNIDAD,w			
			bsf		STATUS,RP0		
			movwf	EEDATA			
			movlw	UNIADDR					
			movwf	EEADR			
			call	wr_eeprom
			bcf		STATUS,RP0			
			
			bsf		INTCON,GIE
			movf    AUXSTATUS,w 	  
			movwf	STATUS            
			movf	AUXW,w		
			retfie

wr_eeprom
			bsf		EECON1,WREN		
			movlw	0x55			
			movwf	EECON2			
			movlw	0xAA			
			movwf	EECON2			
			bsf		EECON1,WR		
ewr			btfsc	EECON1,WR		
			goto	ewr
			bcf		EECON1,WREN	
			return
	
conf
			bsf 	INTCON,GIE
			bsf     INTCON,INTE
			bsf 	STATUS, RP0		
			bcf		OPTION_REG,7		
			movlw 	0x01		
			movwf 	TRISB			
			bcf		TRISA,0			
								;LECTURA DE EEPROM
			movlw	UNIADDR			
			movwf	EEADR			
			bsf		EECON1,RD		
			movf	EEDATA,w			
			bcf 	STATUS,RP0		
			movwf	UNIDAD			
			movlw	0x0A		
			subwf	UNIDAD,w		
			btfsc	STATUS,C		
			clrf	UNIDAD
			
			bsf		STATUS,RP0		
			movlw	DECADDR			
			movwf	EEADR			
			bsf		EECON1,RD				
			movf	EEDATA,w			
			bcf 	STATUS,RP0		
			movwf	DECENA			
			movlw	0x0A		
			subwf	DECENA,w		
			btfsc	STATUS,C		
			clrf	DECENA			
mostrar
			bsf		PORTA,0			
			movf 	UNIDAD,w			
			call	tabla_display				
			movwf 	PORTB										
			call 	delay_display		
			bcf		PORTA,0			
			movf	DECENA,w			
			call 	tabla_display			
			movwf	PORTB			
			call	delay_display
			goto 	mostrar			
							
tabla_display				
			addwf 	PCL,1			
			retlw 	b'01111110'
			retlw 	b'00001100'		
			retlw 	b'10110110'
			retlw 	b'10011110'
			retlw 	b'11001100'
			retlw 	b'11011010'
			retlw 	b'11111010'
			retlw 	b'00001110'
			retlw 	b'11111110'
			retlw 	b'11011110'


delay_1ms	
			movlw 	.251
			movwf	CONT1
loop		decfsz 	CONT1,1
			goto	loop
			return

delay_display
			movlw 	.24
			movwf	CONT2
loop2
			call	delay_1ms
			decfsz	CONT2,1
			goto	loop2		
			return


			END