package de.sve.backend.mail;

import java.util.Arrays;
import java.util.Date;
import java.util.List;
import java.util.Map;
import java.util.Map.Entry;
import java.util.Properties;
import java.util.stream.Collectors;

import javax.activation.DataHandler;
import javax.mail.Address;
import javax.mail.BodyPart;
import javax.mail.Message;
import javax.mail.MessagingException;
import javax.mail.Multipart;
import javax.mail.Session;
import javax.mail.Transport;
import javax.mail.internet.AddressException;
import javax.mail.internet.InternetAddress;
import javax.mail.internet.MimeBodyPart;
import javax.mail.internet.MimeMessage;
import javax.mail.internet.MimeMultipart;

import com.google.common.collect.ImmutableSet;

/**
 * Resposible for sending emails.
 *
 * @author mseele
 */
public class Postman {

	protected static void deliver(Mail mail) throws MessagingException {
		deliver(Arrays.asList(mail));
	}

	protected static void deliver(List<Mail> mails) throws MessagingException {
		deliver(mails.stream().collect(Collectors.groupingBy(Mail::sender)));
	}

	private static void deliver(Map<MailAccount, List<Mail>> mails) throws MessagingException {
		// smtp properties
		Properties properties = System.getProperties();
		String server = "smtp.gmail.com"; //$NON-NLS-1$
		properties.put("mail.smtp.host", server); //$NON-NLS-1$
		properties.put("mail.smtp.port", "465"); //$NON-NLS-1$ //$NON-NLS-2$
		properties.put("mail.smtp.ssl.enable", "true"); //$NON-NLS-1$ //$NON-NLS-2$
		properties.put("mail.smtp.auth", "true"); //$NON-NLS-1$ //$NON-NLS-2$
		for (Entry<MailAccount, List<Mail>> entry : mails.entrySet()) {
			// authenticate
			Session session = Session.getInstance(properties);
			try (Transport transport = session.getTransport("smtp")) { //$NON-NLS-1$
				MailAccount account = entry.getKey();
				transport.connect(server, account.userName(), account.password());
				// send mails
				for (Mail mail : entry.getValue()) {
					MimeMessage message = new MimeMessage(session);
					// recipients
					message.setFrom(new InternetAddress(account.email()));
					InternetAddress[] addresses = addresses(mail.to());
					message.setRecipients(Message.RecipientType.TO, addresses);
					for (String recipient : mail.bcc()) {
						message.addRecipient(Message.RecipientType.BCC, new InternetAddress(recipient));
					}
					String replyTo = mail.replyTo();
					if (replyTo != null) {
						message.setReplyTo(new Address[] { new InternetAddress(replyTo) });
					}
					Multipart multipart = new MimeMultipart();
					// subject & send date
					message.setSubject(mail.subject());
					message.setSentDate(new Date());
					// content
					BodyPart content = new MimeBodyPart();
					content.setText(mail.content());
					multipart.addBodyPart(content);
					// attachments
					for (Attachment attachment : mail.attachments()) {
						BodyPart bodyPart = new MimeBodyPart();
						bodyPart.setDataHandler(new DataHandler(attachment.dataSource()));
						bodyPart.setFileName(attachment.name());
						multipart.addBodyPart(bodyPart);
					}
					message.setContent(multipart);
					message.saveChanges();
					// Send message
					transport.sendMessage(message, addresses);
				}
			}
		}
	}

	private static final InternetAddress[] addresses(ImmutableSet<String> recipients) throws AddressException {
		InternetAddress[] addresses = new InternetAddress[recipients.size()];
		int i = 0;
		for (String recipient : recipients) {
			addresses[i] = new InternetAddress(recipient);
			i++;
		}
		return addresses;
	}

}
