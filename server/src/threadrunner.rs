use scoped_threadpool::Pool;
use crossbeam_channel::{Sender, Receiver, bounded, unbounded};

/// Encapsulates the semantics of distributing tasks across multiple threads.
pub struct Threadrunner<INPUT, OUTPUT> {
    pool: Pool,
    output_sender: Sender<(usize, OUTPUT)>,
    output_receiver: Receiver<(usize, OUTPUT)>,
    operator: fn(&INPUT) -> OUTPUT,
}

impl<INPUT, OUTPUT> Threadrunner<INPUT, OUTPUT> where 
    INPUT: Send + Sync,
    INPUT: std::fmt::Debug,
    OUTPUT: Send + Sync,
    OUTPUT: 'static,
{
    pub fn new(thread_count: u32, operator: fn(&INPUT) -> OUTPUT) -> Threadrunner<INPUT, OUTPUT> {
        let (output_sender, output_receiver) = unbounded();
        Threadrunner {
            pool: Pool::new(thread_count),
            output_sender: output_sender,
            output_receiver: output_receiver,
            operator: operator,
        }
    }
    pub fn new_with_max_batch_size(thread_count: u32, max_batch_size: usize, operator: fn(&INPUT) -> OUTPUT) -> Threadrunner<INPUT, OUTPUT> {
        let (output_sender, output_receiver) = bounded(max_batch_size);
        Threadrunner {
            pool: Pool::new(thread_count),
            output_sender: output_sender,
            output_receiver: output_receiver,
            operator: operator,
        }
    }

    pub fn run_batch<'a>(
        &mut self, 
        jobs: Box<dyn Iterator<Item = &'a INPUT>>
    ) -> Result<Box<dyn Iterator<Item = OUTPUT>>, &str> {
        let mut job_count = 0;
        let mut outputs = Vec::<OUTPUT>::new();

        let initial_operator_instance = self.operator.clone();
        let initial_sender_instance = self.output_sender.clone();

        self.pool.scoped(|scoped| {
            for (index, job) in jobs.enumerate() {
                job_count += 1;
    
                let operator_instance = initial_operator_instance.clone();
                let sender_instance = initial_sender_instance.clone();
                scoped.execute(move || {
                    sender_instance.send((index, operator_instance(job)));
                });
            }

            outputs.reserve(job_count);
            unsafe {
                // Elements are inserted out of order. Batch will fail if not all jobs
                // return a value.
                outputs.set_len(job_count);
            }
        });

        let mut output_count = 0;
        for (index, output) in self.output_receiver.try_iter() {
            output_count += 1;
            outputs[index] = output;
        }

        if output_count < job_count {
            return Err("One or more jobs failed to return an output.");
        }

        return Ok(Box::new(outputs.into_iter()));
    }
}

#[cfg(test)]
mod tests {
    use std::thread::sleep_ms;
    use super::Threadrunner;

    #[test]
    fn processes_batches() {
        let mut runner = Threadrunner::new(4, |num| {
            sleep_ms(*num);
            num * 2
        });

        let results = runner.run_batch(Box::new([1, 4, 3, 10, 8, 1, 4].iter()));
        assert!(results.unwrap().eq(vec![2, 8, 6, 20, 16, 2, 8]));
    }
}
