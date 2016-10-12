.global main
.global init_heap

.global start
start:
and $~7,%rsp
call init_heap
call main
mov $231,%rax # exit_group
syscall

.global getrandom
getrandom:
xor %rdx,%rdx
loop:
mov $318,%rax # getrandom
syscall
test %rax,%rax
js loop
add %rax,%rdi
sub %rax,%rsi
jnz loop
ret

.section .bss
.align 4096
.global HEAP_BOTTOM
HEAP_BOTTOM:
.org .+4*1024*1024
.global HEAP_TOP
HEAP_TOP:
